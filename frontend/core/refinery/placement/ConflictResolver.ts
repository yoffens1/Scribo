// src/core/refinery/placement/ConflictResolver.ts
import type { IFileAccess } from "@utils/_types";
import type { PlacementDecision } from "./types/placement";

/**
 * Resolves filename collisions after placement decisions.
 * If two chunks would create the same file, appends a counter.
 */
export class ConflictResolver {
  constructor(private fileAccess: IFileAccess) {}

  /**
   * Resolve conflicting paths in placement decisions.
   * Deduplicates output paths by appending -1, -2, etc.
   * Skip merge/rename actions — they intentionally target existing paths.
   */
  async resolve(decisions: PlacementDecision[]): Promise<PlacementDecision[]> {
    const claimed = new Map<string, number>(); // path → count of claims
    const resolved: PlacementDecision[] = [];

    for (const d of decisions) {
      let path = d.outputPath;

      // Merge and rename actions INTENTIONALLY target existing paths — don't rename
      if (d.action !== "merge" && d.action !== "rename") {
        // Check disk first
        if (await this.fileAccess.exists(path)) {
          path = await this.findFree(path);
        }

        // Then check batch conflicts
        const taken = claimed.get(path);
        if (taken) {
          path = await this.findFree(this.incrementPath(d.outputPath, taken), claimed);
        }

        claimed.set(path, 1);
      } else {
        // For merge/rename, just mark as seen to track if another action
        // accidentally targets same path
        if (claimed.has(path)) {
          path = await this.findFree(this.incrementPath(d.outputPath, 1), claimed);
          claimed.set(path, 1);
        } else {
          claimed.set(path, 1);
        }
      }

      resolved.push({ ...d, outputPath: path });
    }

    return resolved;
  }

  private async findFree(
    candidate: string,
    claimed?: Map<string, number>,
  ): Promise<string> {
    let path = candidate;
    let suffix = 1;
    while (
      (await this.fileAccess.exists(path)) ||
      (claimed?.has(path))
    ) {
      path = this.incrementPath(candidate, suffix++);
    }
    return path;
  }

  private incrementPath(filePath: string, n: number): string {
    // Find the LAST dot that is AFTER the last slash — only modify filename, not folders
    const lastSlash = filePath.lastIndexOf("/");
    const start = lastSlash === -1 ? 0 : lastSlash + 1;
    const base = filePath.slice(start);
    const dotIndex = base.lastIndexOf(".");

    // Dot at position 0 means it's a dotfile (.gitignore), not an extension
    if (dotIndex <= 0) {
      return `${filePath}-${n}`;
    }
    const nameEnd = start + dotIndex;
    const name = filePath.slice(0, nameEnd);
    const ext = filePath.slice(nameEnd);
    return `${name}-${n}${ext}`;
  }
}
