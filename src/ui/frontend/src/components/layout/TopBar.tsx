import React, { useState, useRef, useEffect } from 'react';
import { useTealDrive, FilterType, SortField } from '../../context/TealDriveContext.tsx';
import {
  Search,
  LayoutGrid,
  List,
  ArrowUpDown,
  ChevronRight,
  User,
  Shield,
  Layers,
  Check,
  Menu,
  LogOut,
} from 'lucide-react';

export const TopBar: React.FC = () => {
  const {
    session,
    currentPath,
    goToPath,
    activeView,
    viewMode,
    setViewMode,
    filter,
    setFilter,
    sortField,
    setSortField,
    sortOrder,
    toggleSortOrder,
    setOmnisearchOpen,
    logout,
    sidebarMobileOpen,
    setSidebarMobileOpen,
  } = useTealDrive();

  const [userDropdownOpen, setUserDropdownOpen] = useState(false);
  const [sortDropdownOpen, setSortDropdownOpen] = useState(false);
  const sortRef = useRef<HTMLDivElement>(null);
  const userRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleOutsideClick = (e: MouseEvent) => {
      if (sortRef.current && !sortRef.current.contains(e.target as Node)) {
        setSortDropdownOpen(false);
      }
      if (userRef.current && !userRef.current.contains(e.target as Node)) {
        setUserDropdownOpen(false);
      }
    };
    document.addEventListener('mousedown', handleOutsideClick);
    return () => document.removeEventListener('mousedown', handleOutsideClick);
  }, []);

  // Parse path segments for breadcrumbs
  const pathParts = currentPath.split('/').filter(Boolean);

  const getBreadcrumbPath = (index: number) => {
    return '/' + pathParts.slice(0, index + 1).join('/');
  };

  return (
    <header className="h-16 px-4 md:px-6 bg-[#090d16]/90 border-b border-slate-800/80 flex items-center justify-between gap-3 backdrop-blur-md sticky top-0 z-10 shrink-0">
      {/* Zone 1: Mobile Menu toggle + Breadcrumbs */}
      <div className="flex items-center gap-2 min-w-0 text-sm font-medium text-slate-200">
        <button
          onClick={() => setSidebarMobileOpen(true)}
          className="p-1.5 rounded-lg text-slate-400 hover:text-slate-100 hover:bg-slate-800 md:hidden shrink-0"
          aria-label="Open sidebar menu"
        >
          <Menu className="w-5 h-5" />
        </button>
        {activeView === 'drive' ? (
          <div className="flex items-center gap-1.5 flex-wrap truncate">
            <button
              onClick={() => goToPath('/')}
              className={`hover:text-teal-400 transition-colors whitespace-nowrap ${
                pathParts.length === 0 ? 'text-teal-400 font-semibold' : 'text-slate-400'
              }`}
            >
              Home
            </button>
            {pathParts.map((part, index) => {
              const isLast = index === pathParts.length - 1;
              const segPath = getBreadcrumbPath(index);
              return (
                <React.Fragment key={segPath}>
                  <ChevronRight className="w-3.5 h-3.5 text-slate-600 shrink-0" />
                  <button
                    onClick={() => goToPath(segPath)}
                    className={`hover:text-teal-400 transition-colors whitespace-nowrap ${
                      isLast ? 'text-teal-300 font-semibold' : 'text-slate-400'
                    }`}
                  >
                    {part}
                  </button>
                </React.Fragment>
              );
            })}
          </div>
        ) : (
          <span className="text-base font-semibold text-slate-100 capitalize">
            {activeView === 'audit' ? 'Audit & Undo Timeline' : `${activeView} Files`}
          </span>
        )}
      </div>

      {/* Zone 2: Omnisearch Trigger */}
      <div className="flex-1 max-w-md mx-1 sm:mx-2 flex justify-center">
        {/* Full search bar on sm+ */}
        <button
          onClick={() => setOmnisearchOpen(true)}
          className="hidden sm:flex w-full items-center justify-between px-3.5 py-1.5 bg-slate-900/90 hover:bg-slate-800 border border-slate-700/60 hover:border-slate-600 rounded-xl text-xs text-slate-400 transition-all shadow-inner group"
        >
          <div className="flex items-center gap-2">
            <Search className="w-3.5 h-3.5 text-slate-500 group-hover:text-teal-400 transition-colors" />
            <span className="truncate">Search files, folders, or contents...</span>
          </div>
          <kbd className="inline-flex items-center gap-0.5 px-1.5 py-0.5 text-[10px] font-mono text-slate-400 bg-slate-800 border border-slate-700 rounded shrink-0">
            <span>⌘</span>
            <span>K</span>
          </kbd>
        </button>

        {/* Compact search icon button on mobile */}
        <button
          onClick={() => setOmnisearchOpen(true)}
          className="sm:hidden p-2 rounded-lg bg-slate-900 border border-slate-800 text-slate-400 hover:text-teal-300"
          aria-label="Search"
        >
          <Search className="w-4 h-4 text-teal-400" />
        </button>
      </div>

      {/* Zone 3: Actions & Controls */}
      <div className="flex items-center gap-2 shrink-0">
        {/* View Toggle (Grid vs Table) */}
        {activeView === 'drive' && (
          <div className="flex items-center p-0.5 bg-slate-900 border border-slate-800 rounded-lg">
            <button
              onClick={() => setViewMode('grid')}
              title="Grid View"
              className={`p-1.5 rounded-md transition-colors ${
                viewMode === 'grid'
                  ? 'bg-slate-800 text-teal-400 shadow-sm'
                  : 'text-slate-400 hover:text-slate-200'
              }`}
            >
              <LayoutGrid className="w-4 h-4" />
            </button>
            <button
              onClick={() => setViewMode('table')}
              title="Table View"
              className={`p-1.5 rounded-md transition-colors ${
                viewMode === 'table'
                  ? 'bg-slate-800 text-teal-400 shadow-sm'
                  : 'text-slate-400 hover:text-slate-200'
              }`}
            >
              <List className="w-4 h-4" />
            </button>
          </div>
        )}

        {/* Sort Selector */}
        {activeView === 'drive' && (
          <div className="relative">
            <button
              onClick={() => setSortDropdownOpen(!sortDropdownOpen)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 bg-slate-900 border border-slate-800 hover:border-slate-700 rounded-lg text-xs text-slate-300 transition-colors"
            >
              <ArrowUpDown className="w-3.5 h-3.5 text-teal-400" />
              <span className="hidden sm:inline capitalize">{sortField}</span>
            </button>

            {sortDropdownOpen && (
              <div className="absolute right-0 top-10 w-44 bg-slate-900 border border-slate-700 rounded-xl shadow-xl p-1.5 z-40 text-xs text-slate-300 animate-in fade-in zoom-in-95 duration-100">
                <div className="px-2 py-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wider">
                  Sort By
                </div>
                {(['name', 'size', 'modified'] as SortField[]).map((field) => (
                  <button
                    key={field}
                    onClick={() => {
                      setSortField(field);
                      setSortDropdownOpen(false);
                    }}
                    className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-left transition-colors ${
                      sortField === field ? 'bg-teal-500/10 text-teal-300 font-semibold' : 'hover:bg-slate-800'
                    }`}
                  >
                    <span className="capitalize">{field}</span>
                    {sortField === field && <Check className="w-3.5 h-3.5 text-teal-400" />}
                  </button>
                ))}
                <div className="h-px bg-slate-800 my-1" />
                <button
                  onClick={() => {
                    toggleSortOrder();
                    setSortDropdownOpen(false);
                  }}
                  className="w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-left hover:bg-slate-800 text-slate-300"
                >
                  <span>Order: {sortOrder === 'asc' ? 'Ascending' : 'Descending'}</span>
                </button>
              </div>
            )}
          </div>
        )}

        {/* User Profile Menu */}
        <div className="relative">
          <button
            onClick={() => setUserDropdownOpen(!userDropdownOpen)}
            className="flex items-center gap-2 p-1 pl-2 bg-slate-900 hover:bg-slate-800 border border-slate-800 hover:border-slate-700 rounded-full transition-colors"
          >
            <span className="text-xs font-semibold text-slate-200 hidden md:inline">
              {session?.username || 'User'}
            </span>
            <div className="w-7 h-7 rounded-full bg-gradient-to-tr from-teal-500 to-teal-400 flex items-center justify-center text-slate-950 font-bold text-xs shadow">
              {(session?.username || 'U')[0].toUpperCase()}
            </div>
          </button>

          {/* User Popover */}
          {userDropdownOpen && (
            <div className="absolute right-0 top-11 w-56 bg-slate-900 border border-slate-700 rounded-xl shadow-2xl p-2 z-40 text-xs text-slate-300 animate-in fade-in zoom-in-95 duration-100">
              <div className="p-2 border-b border-slate-800">
                <div className="font-semibold text-slate-100 flex items-center justify-between">
                  <span>{session?.username}</span>
                  {session?.isAdmin && (
                    <span className="px-1.5 py-0.5 rounded text-[10px] font-mono bg-purple-950 border border-purple-700 text-purple-300 flex items-center gap-1">
                      <Shield className="w-2.5 h-2.5" />
                      Admin
                    </span>
                  )}
                </div>
                {session?.email && <div className="text-[11px] text-slate-400 truncate mt-0.5">{session.email}</div>}
                {session?.uid !== undefined && <div className="text-[10px] font-mono text-slate-500 mt-1">UID: {session.uid}</div>}
              </div>

              <div className="py-1">
                <button
                  onClick={() => {
                    logout();
                    setUserDropdownOpen(false);
                  }}
                  className="w-full flex items-center gap-2 px-2.5 py-2 rounded-lg text-left text-xs text-rose-300 hover:bg-rose-500/10 hover:text-rose-200 transition-colors"
                >
                  <LogOut className="w-3.5 h-3.5 text-rose-400" />
                  <span>Sign Out</span>
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </header>
  );
};
