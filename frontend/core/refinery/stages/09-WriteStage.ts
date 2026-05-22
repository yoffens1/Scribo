import { invoke } from "@tauri-apps/api/core";
// src/core/refinery/stages/WriteStage.ts
import * as path from "path";
import type { RefineryStage } from "../types/refinery-stage";
import type { RefineryContext } from "../types/refinery-context";
import type { WriteOperation } from "../types/refinery-result";
import type { ChunkWithHash } from "../types/chunk-decision";
import type { AtomChunk } from "../types/atom-chunk";
import { LogScope } from "../types/refinery-stage";
import { FileWriter } from "../writers/FileWriter";
import { TransactionalWriter } from "../writers/TransactionalWriter";

export interface WriteInput {
  plan: { decisions: import("../placement/types/placement").PlacementDecision[] };
  chunks: ChunkWithHash[];
  dryRun?: boolean;
  sourcePath: string;
}

const ensureMd = (p: string): string => p.endsWith(".md") ? p : `${p}.md`;

const buildFrontmatter = (chunk: ChunkWithHash & Partial<AtomChunk>): string => {
  const lines: string[] = ["---"];
  if (chunk.aliases && chunk.aliases.length > 0) {
    const aliasStr = chunk.aliases.map(a => JSON.stringify(a)).join(", ");
    lines.push(`aliases: [${aliasStr}]`);
  }
  if (chunk.tags && chunk.tags.length > 0) {
    const tagStr = chunk.tags.map(t => JSON.stringify(t)).join(", ");
    lines.push(`tags: [${tagStr}]`);
  }
  if (chunk.sources && chunk.sources.length > 0) {
    const srcStr = chunk.sources.map(s => JSON.stringify(s)).join(", ");
    lines.push(`sources: [${srcStr}]`);
  }
  lines.push("---");
  lines.push("");
  return lines.join("\n");
};

const buildContent = (chunk: ChunkWithHash & Partial<AtomChunk>): string => {
  const fm = buildFrontmatter(chunk);
  let text = chunk.generationText;
  
  if (chunk.questionHeading) {
    // Удаляем оригинальный заголовок, если он есть в начале текста, чтобы избежать дублирования
    text = text.replace(/^\s*#+\s+.*?(?:\r?\n|$)/, "").trimStart();
    return fm + chunk.questionHeading + "\n" + text;
  }
  return fm + text;
};

const derivePath = (
  ctx: RefineryContext,
  decision: import("../placement/types/placement").PlacementDecision,
  chunk: ChunkWithHash & Partial<AtomChunk>,
): string => {
  let relativePath = decision.outputPath;
  if (relativePath.startsWith(ctx.outputRoot + "/")) {
    relativePath = relativePath.slice(ctx.outputRoot.length + 1);
  }
  
  if (chunk.filename) {
    return path.posix.join(ctx.outputRoot, path.posix.dirname(relativePath), chunk.filename);
  }
  return path.posix.join(ctx.outputRoot, ensureMd(relativePath));
};

export class WriteStage implements RefineryStage<WriteInput, WriteOperation[]> {
  readonly name = "WriteStage";

  async run(input: WriteInput, ctx: RefineryContext): Promise<WriteOperation[]> {
    const writer = new FileWriter(ctx);
    const txWriter = new TransactionalWriter(writer, ctx.fileAccess, ctx.logger);
    const operations = await this.buildOperations(input, ctx);

    let sourceFileId: number | null = null;
    const dbActive = !!ctx.dbConnection; // Wait, actually ctx.dbConnection means DB is active.
    if (dbActive) {
      const srcRecord = await invoke<import("../../database/models/types").FileRecord | null>("files_get_by_path", { path: input.sourcePath });
      if (srcRecord) {
        sourceFileId = srcRecord.fileId;
      } else {
        const newId = await invoke<number>("files_insert_minimal", {
          path: input.sourcePath,
          name: path.basename(input.sourcePath),
          hash: ""
        });
        sourceFileId = newId;
      }
    }

    if (input.dryRun ?? ctx.dryRun) {
      ctx.logger.log("info", LogScope.WRITE_DRYRUN, "dry run — no files written", { operationCount: operations.length });
      return operations;
    }

    ctx.logger.log("info", LogScope.WRITE_START, `executing ${operations.length} operations`);
    await txWriter.executeBatch(operations, sourceFileId);
    ctx.logger.log("info", LogScope.WRITE_DONE, "all operations complete");
    return operations;
  }

  private async buildOperations(input: WriteInput, ctx: RefineryContext): Promise<WriteOperation[]> {
    const chunkMap = new Map(input.chunks.map((c) => [c.hash, c]));
    const neededFolders = new Set<string>();
    const fileOps: WriteOperation[] = [];
    const writtenPaths = new Set<string>();

    for (const decision of input.plan.decisions) {
      const chunk = chunkMap.get(decision.chunkHash);
      if (!chunk) continue;

      const atomChunk = chunk as ChunkWithHash & Partial<AtomChunk>;
      const fullPath = derivePath(ctx, decision, atomChunk);
      const content = buildContent(atomChunk);
      const dir = path.posix.dirname(fullPath);
      if (dir && dir !== "." && dir !== ctx.outputRoot) {
        neededFolders.add(dir);
      }

      switch (decision.action) {
        case "create":
          fileOps.push({ type: "create_file", path: fullPath, content });
          writtenPaths.add(fullPath);
          break;
        case "merge":
        case "rename": {
          let target = decision.existingTarget
            ? ensureMd(decision.existingTarget)
            : fullPath;
          if (!target.startsWith(ctx.outputRoot + "/")) {
            target = path.posix.join(ctx.outputRoot, target);
          }

          if (decision.action === "rename") {
            if (target !== fullPath) {
              fileOps.push({ type: "move_file", from: target, to: fullPath });
            }
            writtenPaths.add(fullPath);
            fileOps.push({ type: "merge_chunk", sourceFile: atomChunk.sourcePath, targetFile: fullPath, chunkText: content });
          } else {
            writtenPaths.add(target);
            fileOps.push({ type: "merge_chunk", sourceFile: atomChunk.sourcePath, targetFile: target, chunkText: content });
          }
          break;
        }
        case "nest": {
          const nestDir = path.posix.dirname(fullPath);
          if (nestDir !== ctx.outputRoot) neededFolders.add(nestDir);
          fileOps.push({ type: "merge_chunk", sourceFile: atomChunk.sourcePath, targetFile: fullPath, chunkText: content });
          writtenPaths.add(fullPath);
          break;
        }
      }
    }

    let sourceFileId: number | null = null;
    const dbActive = !!ctx.dbConnection;
    if (dbActive) {
      const srcRecord = await invoke<import("../../database/models/types").FileRecord | null>("files_get_by_path", { path: input.sourcePath });
      if (srcRecord) {
        sourceFileId = srcRecord.fileId;
      }
    }

    if (dbActive && sourceFileId !== null) {
      const oldFiles = await invoke<string[]>("files_get_by_source_file_id", { sourceFileId });
      for (const oldFile of oldFiles) {
        if (!writtenPaths.has(oldFile)) {
          fileOps.push({ type: "delete_file", path: oldFile });
        }
      }
    }

    const operations: WriteOperation[] = [];
    for (const folder of neededFolders) {
      operations.push({ type: "create_folder", path: folder });
    }
    operations.push(...fileOps);
    return operations;
  }
}
