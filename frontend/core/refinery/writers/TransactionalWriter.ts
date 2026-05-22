// src/core/refinery/writers/TransactionalWriter.ts
import type { Logger } from "@logging/Logger";
import type { IFileAccess } from "@utils/_types";
import type { WriteOperation } from "../types/refinery-result";
import { LogScope } from "../types/refinery-stage";
import { FileWriter } from "./FileWriter";

export class TransactionalWriter {
  private executed: WriteOperation[] = [];
  private mergeBackups = new Map<string, string>();

  constructor(
    private writer: FileWriter,
    private fileAccess: IFileAccess,
    private logger: Logger,
  ) {}

  async executeBatch(operations: WriteOperation[], sourceFileId: number | null): Promise<void> {
    this.executed = [];
    this.mergeBackups.clear();

    try {
      for (const op of operations) {
        if (op.type === "merge_chunk" && await this.fileAccess.exists(op.targetFile)) {
          this.mergeBackups.set(op.targetFile, await this.fileAccess.readText(op.targetFile));
        } else if (op.type === "delete_file" && await this.fileAccess.exists(op.path)) {
          this.mergeBackups.set(op.path, await this.fileAccess.readText(op.path));
        }
        await this.writer.execute(op, sourceFileId);
        this.executed.push(op);
      }
      this.logger.log("info", LogScope.WRITER_BATCH, "batch complete", { totalOps: operations.length });
    } catch (err) {
      const errorMsg = err instanceof Error ? err.stack ?? err.message : String(err);
      this.logger.log("error", LogScope.WRITER_BATCH, "batch failed, rolling back", {
        error: errorMsg, executedCount: this.executed.length,
      });
      await this.rollback();
      throw err;
    }
  }

  private async rollback(): Promise<void> {
    for (const op of [...this.executed].reverse()) {
      try { await this.rollbackOp(op); } catch (err) {
        this.logger.log("warn", LogScope.WRITER_ROLLBACK, "rollback failed for op", {
          op: op.type, error: String(err),
        });
      }
    }
  }

  private async rollbackOp(op: WriteOperation): Promise<void> {
    switch (op.type) {
      case "create_file":
        await this.fileAccess.delete(op.path);
        break;
      case "merge_chunk": {
        const backup = this.mergeBackups.get(op.targetFile);
        if (backup !== undefined) {
          await this.fileAccess.writeText(op.targetFile, backup);
        } else {
          await this.fileAccess.delete(op.targetFile);
        }
        break;
      }
      case "move_file":
        try { await this.fileAccess.rename(op.to, op.from); } catch {
          this.logger.log("warn", LogScope.WRITER_ROLLBACK, "cannot undo move", { from: op.from, to: op.to });
        }
        break;
      case "create_folder":
        break;
      case "delete_file": {
        const backup = this.mergeBackups.get(op.path);
        if (backup !== undefined) {
          await this.fileAccess.writeText(op.path, backup);
        }
        break;
      }
    }
  }
}
