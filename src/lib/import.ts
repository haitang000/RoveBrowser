export interface ImportSource { id: string; browser: string; name: string; bookmarks: boolean; history: boolean }
export interface ImportReport { id: string; name: string; bookmarks: number; history: number; passwords: number; skipped: number; problems: string[] }
export interface ImportRequest { sources: string[]; bookmarks: boolean; history: boolean }
