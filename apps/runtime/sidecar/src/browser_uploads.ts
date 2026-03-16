import { copyFile, mkdir } from 'node:fs/promises';
import path from 'node:path';

const OPENCLAW_UPLOAD_PREFIX = '/tmp/openclaw/uploads/';

export function mapCompatUploadPath(inputPath: string, stagingRoot: string): string {
  return path.join(stagingRoot, 'openclaw', 'uploads', path.basename(inputPath));
}

export async function stageCompatUploadPaths(paths: string[], stagingRoot: string): Promise<string[]> {
  const staged: string[] = [];

  for (const inputPath of paths) {
    const destination = mapCompatUploadPath(inputPath, stagingRoot);
    await mkdir(path.dirname(destination), { recursive: true });

    if (!inputPath.startsWith(OPENCLAW_UPLOAD_PREFIX)) {
      await copyFile(inputPath, destination);
    }

    staged.push(destination);
  }

  return staged;
}
