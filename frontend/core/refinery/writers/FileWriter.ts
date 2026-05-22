// src/core/refinery/writers/FileWriter.ts
import * as path from "path";
import { invoke } from "@tauri-apps/api/core";
import type { WriteOperation } from "../types/refinery-result";
import type { RefineryContext } from "../types/refinery-context";
import { LogScope } from "../types/refinery-stage";
import { buildChunkMergePrompt } from "@ai/prompts/refinery/chunk-merge";
import { HashService } from "../../database/services/indexing/HashService";

export class FileWriter {
  private hashService = new HashService();

  constructor(
    private ctx: RefineryContext,
  ) {}

  async execute(op: WriteOperation, sourceFileId: number | null): Promise<void> {
    switch (op.type) {
      case "create_file":
        await this.ctx.fileAccess.writeText(op.path, op.content);
        this.ctx.logger.log("debug", LogScope.WRITER_CREATE_FILE, op.path, { contentLength: op.content.length });
        await this.syncDatabase(op.path, op.content, sourceFileId);
        break;

      case "merge_chunk": {
        const mergedContent = await this.mergeCardContent(op.targetFile, op.chunkText);
        await this.ctx.fileAccess.writeText(op.targetFile, mergedContent);
        await this.syncDatabase(op.targetFile, mergedContent, sourceFileId);
        break;
      }

      case "create_folder":
        this.ctx.logger.log("debug", LogScope.WRITER_CREATE_FOLDER, op.path);
        break;

      case "move_file":
        this.ctx.logger.log("info", LogScope.WRITER_MOVE_FILE, `${op.from} → ${op.to}`);
        await this.ctx.fileAccess.rename(op.from, op.to);
        if (this.ctx.dbConnection) {
          await invoke("files_rename", { oldPath: op.from, newPath: op.to, updatedAt: Date.now() });
        }
        break;

      case "delete_file":
        this.ctx.logger.log("info", "writer.delete_file", op.path);
        if (await this.ctx.fileAccess.exists(op.path)) {
          await this.ctx.fileAccess.delete(op.path);
        }
        if (this.ctx.dbConnection) {
          if (this.ctx.deleteFromDbOnGc) {
            await invoke("files_hard_delete", { path: op.path });
          } else {
            await invoke("files_soft_delete", { path: op.path, updatedAt: Date.now() });
          }
        }
        break;
    }
  }

  private async mergeCardContent(targetPath: string, newContentWithFm: string): Promise<string> {
    let existingContentWithFm = "";
    try {
      existingContentWithFm = await this.ctx.fileAccess.readText(targetPath);
    } catch {
      return newContentWithFm;
    }

    const existingFmMatch = existingContentWithFm.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
    let existingFm = "";
    let existingBody = existingContentWithFm;
    if (existingFmMatch) {
      existingFm = existingFmMatch[1];
      existingBody = existingFmMatch[2];
    }

    const newFmMatch = newContentWithFm.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
    let newFm = "";
    let newBody = newContentWithFm;
    if (newFmMatch) {
      newFm = newFmMatch[1];
      newBody = newFmMatch[2];
    }

    let mergedBody = "";
    if (this.ctx.overwriteOnMerge) {
      const messages = buildChunkMergePrompt(existingBody, newBody);
      const response = await this.ctx.llm.generateMessages(messages);
      mergedBody = response.text.trim();
    } else {
      mergedBody = existingBody.trim() + "\n\n" + newBody.trim();
    }

    let mergedFm = newFm;
    if (this.ctx.mergeTags) {
      const parseList = (fm: string, key: string): string[] => {
        const regex = new RegExp(`^${key}:\\s*\\[?(.*?)\\]?\\s*$`, "m");
        const match = fm.match(regex);
        if (!match) return [];
        return match[1]
          .split(",")
          .map(s => s.trim().replace(/^["']|["']$/g, ""))
          .filter(Boolean);
      };

      const existingAliases = parseList(existingFm, "aliases");
      const newAliases = parseList(newFm, "aliases");
      const mergedAliases = Array.from(new Set([...existingAliases, ...newAliases]));

      const existingTags = parseList(existingFm, "tags");
      const newTags = parseList(newFm, "tags");
      const mergedTags = Array.from(new Set([...existingTags, ...newTags]));

      const existingSources = parseList(existingFm, "sources");
      const newSources = parseList(newFm, "sources");
      const mergedSources = Array.from(new Set([...existingSources, ...newSources]));

      const fmLines = ["---"];
      if (mergedAliases.length > 0) {
        fmLines.push(`aliases: [${mergedAliases.map(a => JSON.stringify(a)).join(", ")}]`);
      }
      if (mergedTags.length > 0) {
        fmLines.push(`tags: [${mergedTags.map(t => JSON.stringify(t)).join(", ")}]`);
      }
      if (mergedSources.length > 0) {
        fmLines.push(`sources: [${mergedSources.map(s => JSON.stringify(s)).join(", ")}]`);
      }
      fmLines.push("---");
      fmLines.push("");
      mergedFm = fmLines.join("\n");
    } else {
      mergedFm = newFm ? `---\n${newFm}\n---\n\n` : "";
    }

    if (mergedFm.startsWith("---")) {
      return mergedFm + mergedBody;
    }
    return mergedFm ? `---\n${mergedFm}\n---\n\n` + mergedBody : mergedBody;
  }

  private async syncDatabase(filePath: string, content: string, sourceFileId: number | null): Promise<void> {
    if (!this.ctx.dbConnection) return;

    const cleanPath = filePath;
    const fileName = path.basename(filePath);
    const fileHashVal = await this.hashService.compute(content);
    const mtime = Date.now();

    const fileId = await invoke<number>("files_sync_upsert", {
      path: cleanPath,
      name: fileName,
      hash: fileHashVal,
      mtime,
      sourceFileId
    });

    await invoke("cards_insert_ignore", { fileId });
  }
}
