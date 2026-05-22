// src/utils/_types.ts

export interface IFileAccess {
  readBinary(normalizedPath: string): Promise<ArrayBuffer>;
  readText(normalizedPath: string): Promise<string>;
  writeText(normalizedPath: string, content: string): Promise<void>;
  exists(normalizedPath: string): Promise<boolean>;
  /** List immediate children (files + folders) of a directory. */
  list(normalizedPath: string): Promise<Array<{ name: string; isDir: boolean }>>;
  /** Rename or move a file/folder. */
  rename(from: string, to: string): Promise<void>;
  /** Delete a file. */
  delete(normalizedPath: string): Promise<void>;
}
