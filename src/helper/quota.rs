//! Root-scoped, cross-process quota reservations. The lock stays held until commit
//! or rollback. Initial usage is measured before dropping elevated privileges.
use std::cell::Cell;
use std::fs::File;
use std::io::{self,Write,Seek,SeekFrom};
use std::os::fd::AsRawFd;
use crate::config::AllowedRoot;
thread_local! { static BUDGET:Cell<Option<u64>>=const { Cell::new(None) }; }
pub fn budget()->Option<u64>{BUDGET.with(Cell::get)}
pub fn with_budget<T>(available:Option<u64>,f:impl FnOnce()->T)->T {
    struct Reset(Option<u64>);impl Drop for Reset{fn drop(&mut self){BUDGET.with(|b|b.set(self.0));}}
    let _reset=Reset(BUDGET.with(|b|b.replace(available)));f()
}
pub struct Reservation { _directory:File,pub available:Option<u64> }
impl Reservation {
    pub fn acquire(root:&AllowedRoot)->io::Result<Self> {
        let file=crate::helper::sandbox::open(root,&crate::config::RelativePath::new(""),libc::O_RDONLY|libc::O_DIRECTORY,0)?;
        if unsafe {libc::flock(file.as_raw_fd(),libc::LOCK_EX)}!=0{return Err(io::Error::last_os_error());}
        let available=match root.quota_bytes {Some(limit)=>Some(limit.saturating_sub(size(&file,0)?)),None=>None};
        Ok(Self{_directory:file,available})
    }
}
fn size(directory:&File,depth:usize)->io::Result<u64> {
    if depth>128{return Err(io::Error::other("quota traversal depth exceeded"));}
    let mut total=0u64;
    for entry in std::fs::read_dir(format!("/proc/self/fd/{}",directory.as_raw_fd()))? {
        let entry=entry?;let name=entry.file_name();let name=name.to_str().ok_or_else(||io::Error::other("non UTF-8 quota entry"))?;
        let file=crate::helper::sandbox::open_at(directory.as_raw_fd(),name,libc::O_PATH,0)?;
        let metadata=file.metadata()?;
        if metadata.is_symlink(){continue;}
        let count=if metadata.is_dir(){
            let child=crate::helper::sandbox::open_at(directory.as_raw_fd(),name,libc::O_RDONLY|libc::O_DIRECTORY,0)?;
            size(&child,depth+1)?
        }else if metadata.is_file(){metadata.len()}else{0};
        total=total.checked_add(count).ok_or_else(||io::Error::other("quota overflow"))?;
    }
    Ok(total)
}
/// Seek-aware bound for archives, so rewriting ZIP headers does not count twice.
pub struct BudgetWriter<W>{inner:W,limit:u64,position:u64}
impl<W> BudgetWriter<W>{pub fn new(inner:W)->Self{Self{inner,limit:budget().unwrap_or(u64::MAX),position:0}}}
impl<W:Write> Write for BudgetWriter<W>{
    fn write(&mut self,bytes:&[u8])->io::Result<usize>{
        if bytes.len() as u64>self.limit.saturating_sub(self.position){return Err(io::Error::from_raw_os_error(libc::ENOSPC));}
        let count=self.inner.write(bytes)?;self.position+=count as u64;Ok(count)
    }
    fn flush(&mut self)->io::Result<()>{self.inner.flush()}
}
impl<W:Seek> Seek for BudgetWriter<W>{fn seek(&mut self,position:SeekFrom)->io::Result<u64>{let p=self.inner.seek(position)?;self.position=p;Ok(p)}}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn cross_process_reservation_counts_existing_files(){
        let base=std::env::temp_dir().join(format!("tdrv-reservation-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();std::fs::write(base.join("existing"),[0u8;80]).unwrap();
        let root=AllowedRoot{base_path:base.clone(),quota_bytes:Some(100),..Default::default()};
        let guard=Reservation::acquire(&root).unwrap();assert_eq!(guard.available,Some(20));
        with_budget(guard.available,||{let mut writer=BudgetWriter::new(Vec::new());assert!(writer.write_all(&[0u8;21]).is_err());});
        drop(guard);std::fs::remove_dir_all(base).unwrap();
    }
}
