// src/core/refinery/taxonomy/FolderTreeReader.ts
import type { IFileAccess } from "@utils/_types";

/**
 * Reads the current output folder structure for LLM placement context.
 * Uses IFileAccess.list() for BFS traversal.
 */
export class FolderTreeReader {
  constructor(private fileAccess: IFileAccess) {}

  /**
   * Scan the output root and return a tree string like:
   *   /
   *   ├── programming/
   *   │   ├── python.md
   *   │   └── rust.md
   *   └── philosophy/
   */
  async readTree(rootPath: string): Promise<string> {
    const lines = await this.walk(rootPath, "");
    if (lines.length === 0) return "/\n  (empty)";
    return lines.join("\n");
  }

  /** List all folder paths under root (relative, slash-separated). */
  async listFolders(rootPath: string): Promise<string[]> {
    const folders: string[] = [];
    await this.collectFolders(rootPath, "", folders);
    return folders;
  }

  private async collectFolders(
    absPath: string,
    relPath: string,
    result: string[],
  ): Promise<void> {
    const entries = await this.fileAccess.list(absPath);
    for (const entry of entries) {
      if (!entry.isDir) continue;
      const childRel = relPath ? `${relPath}/${entry.name}` : entry.name;
      result.push(childRel);
      const childAbs = absPath ? `${absPath}/${entry.name}` : entry.name;
      await this.collectFolders(childAbs, childRel, result);
    }
  }

  private async walk(
    absPath: string,
    relPath: string,
  ): Promise<string[]> {
    const entries = await this.fileAccess.list(absPath || ".");
    if (entries.length === 0) {
      return relPath ? [`${relPath}/`] : [];
    }

    // Sort: folders first, then files, alphabetically
    const sorted = [...entries].sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
      return a.name.localeCompare(b.name);
    });

    const lines: string[] = [];
    if (relPath) {
      lines.push(`${relPath}/`);
    }

    for (let i = 0; i < sorted.length; i++) {
      const e = sorted[i];
      const isLast = i === sorted.length - 1;
      const prefix = isLast ? "└── " : "├── ";
      const childRel = relPath ? `${relPath}/${e.name}` : e.name;
      const childAbs = absPath ? `${absPath}/${e.name}` : e.name;

      if (e.isDir) {
        const childLines = await this.walk(childAbs, childRel);
        if (childLines.length > 0) {
          // First line is the folder itself
          lines.push(`${prefix}${e.name}/`);
          const indent = isLast ? "    " : "│   ";
          lines.push(...childLines.map(l => indent + l));
        } else {
          lines.push(`${prefix}${e.name}/ (empty)`);
        }
      } else {
        lines.push(`${prefix}${e.name}`);
      }
    }

    return lines;
  }
}
