export interface PermissionTriplet {
  read: boolean;
  write: boolean;
  execute: boolean;
}

export interface PermissionMatrix {
  owner: PermissionTriplet;
  group: PermissionTriplet;
  others: PermissionTriplet;
}

/**
 * Converts a numeric octal string (e.g., "755" or "0755") or POSIX integer (e.g. 493 or 0o755) to a PermissionMatrix
 */
export function octalToMatrix(mode: string | number): PermissionMatrix {
  const clean =
    typeof mode === 'number'
      ? (mode & 0o777).toString(8).padStart(3, '0')
      : String(mode).replace(/^0+/, '').padStart(3, '0').slice(-3);
  
  const parseDigit = (d: string): PermissionTriplet => {
    const val = parseInt(d, 8) || 0;
    return {
      read: (val & 4) !== 0,
      write: (val & 2) !== 0,
      execute: (val & 1) !== 0,
    };
  };

  return {
    owner: parseDigit(clean[0]),
    group: parseDigit(clean[1]),
    others: parseDigit(clean[2]),
  };
}

/**
 * Converts a PermissionMatrix to a 4-digit octal string (e.g., "0755")
 */
export function matrixToOctal(matrix: PermissionMatrix): string {
  const tripletToDigit = (t: PermissionTriplet): number => {
    let sum = 0;
    if (t.read) sum += 4;
    if (t.write) sum += 2;
    if (t.execute) sum += 1;
    return sum;
  };

  const o = tripletToDigit(matrix.owner);
  const g = tripletToDigit(matrix.group);
  const ot = tripletToDigit(matrix.others);

  return `0${o}${g}${ot}`;
}

/**
 * Converts a mode string or number (e.g. "0755" or 0o755) to standard UNIX symbolic format (e.g. "rwxr-xr-x")
 */
export function modeToSymbolic(mode: string | number, isDirectory = false): string {
  const matrix = octalToMatrix(mode);
  const tripletToSym = (t: PermissionTriplet) =>
    `${t.read ? 'r' : '-'}${t.write ? 'w' : '-'}${t.execute ? 'x' : '-'}`;

  const prefix = isDirectory ? 'd' : '-';
  return `${prefix}${tripletToSym(matrix.owner)}${tripletToSym(matrix.group)}${tripletToSym(matrix.others)}`;
}

/**
 * Returns security hints or warnings for a given mode
 */
export function checkPermissionSecurity(matrix: PermissionMatrix): { isSafe: boolean; warning?: string } {
  if (matrix.others.write) {
    return {
      isSafe: false,
      warning: 'World-writable: Any local system or virtual user can overwrite or delete this file.',
    };
  }
  if (matrix.group.write && !matrix.owner.write) {
    return {
      isSafe: false,
      warning: 'Group writable but owner read-only: Unusual configuration.',
    };
  }
  return { isSafe: true };
}
