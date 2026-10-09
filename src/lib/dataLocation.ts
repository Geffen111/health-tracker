// Types for the data folder and startup (commands/data_location.rs).

export interface LockInfo {
  machine: string;
  /** Unix seconds. */
  opened: number;
  heartbeat: number;
}

export interface StartupInfo {
  status: 'ready' | 'needs_location' | 'locked' | 'error';
  data_dir: string | null;
  lock: LockInfo | null;
  error: string | null;
  machine: string;
}

export interface LocationOption {
  kind: 'local' | 'cloud';
  label: string;
  path: string;
  has_data: boolean;
  modified: string | null;
}

export interface DataLocation {
  path: string;
  machine: string;
  stray_copies: string[];
}
