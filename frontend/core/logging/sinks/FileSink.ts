// src/core/logging/sinks/FileSink.ts
import type { Sink } from "../types/sink";
import type { LogEvent, Trace } from "../types";
import * as fs from "fs/promises";
import * as path from "path";

/**
 * Writes traces as JSON files to a directory on disk.
 *
 * By default uses `fs/promises` — inject `writeFile` and `mkdir` functions
 * for testing (e.g. in-memory Map<string, string>).
 */
export class FileSink implements Sink {
  private writeFile: (filePath: string, content: string) => Promise<void>;
  private mkdir: (dirPath: string) => Promise<void>;

  constructor(
    private dir: string,
    writeFile?: (filePath: string, content: string) => Promise<void>,
    mkdir?: (dirPath: string) => Promise<void>,
  ) {
    this.writeFile = writeFile ?? ((p, c) => fs.writeFile(p, c, "utf-8"));
    this.mkdir = mkdir ?? (async (d) => { await fs.mkdir(d, { recursive: true }); });
  }

  write(_e: LogEvent): void {}

  async flushTrace(t: Trace): Promise<void> {
    await this.mkdir(this.dir);
    const file = path.join(
      this.dir,
      `trace-${Math.floor(t.startedAt)}-${t.id.slice(0, 8)}.json`,
    );
    await this.writeFile(file, JSON.stringify(t, null, 2));
    console.log(`Trace saved: ${file}`);
  }
}
