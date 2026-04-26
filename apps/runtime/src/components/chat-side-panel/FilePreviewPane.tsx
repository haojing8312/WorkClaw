import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

export type WorkspaceFilePreview = {
  path: string;
  kind: "text" | "markdown" | "html" | "image" | "binary" | "directory" | "docx";
  source?: string;
  size?: number;
  truncated?: boolean;
  previewError?: string;
};

interface FilePreviewPaneProps {
  preview: WorkspaceFilePreview | null;
  mode: "rendered" | "source";
  workspace?: string;
  onModeChange: (mode: "rendered" | "source") => void;
  onCopyPath?: (path: string) => void;
  onOpenFile?: (path: string) => void;
}

function toFileHref(workspace: string, relativePath: string): string {
  const normalizedWorkspace = workspace.replace(/\\/g, "/").replace(/\/+$/, "");
  const normalizedPath = relativePath.replace(/\\/g, "/").replace(/^\/+/, "");
  const directoryPath = normalizedPath.includes("/")
    ? normalizedPath.slice(0, normalizedPath.lastIndexOf("/") + 1)
    : "";
  const fullDirectoryPath = `${normalizedWorkspace}/${directoryPath}`.replace(/\/+/g, "/");
  const withLeadingSlash = fullDirectoryPath.startsWith("/") ? fullDirectoryPath : `/${fullDirectoryPath}`;
  const withTrailingSlash = withLeadingSlash.endsWith("/") ? withLeadingSlash : `${withLeadingSlash}/`;
  return encodeURI(`file://${withTrailingSlash}`);
}

function buildHtmlPreviewSource(workspace: string | undefined, preview: WorkspaceFilePreview): string {
  const source = preview.source || "";
  if (!workspace || preview.kind !== "html") return source;
  const baseHref = toFileHref(workspace, preview.path);
  if (source.includes("<head")) {
    return source.replace(/<head([^>]*)>/i, `<head$1><base href="${baseHref}">`);
  }
  return `<head><base href="${baseHref}"></head>${source}`;
}

export function FilePreviewPane({
  preview,
  mode,
  workspace,
  onModeChange,
  onCopyPath,
  onOpenFile,
}: FilePreviewPaneProps) {
  if (!preview) {
    return <div className="flex h-full items-center justify-center text-3xl font-semibold text-gray-300">选择要查看的文件</div>;
  }

  if (preview.kind === "directory") {
    return <div className="flex h-full items-center justify-center text-xl font-semibold text-gray-300">选择目录中的文件进行预览</div>;
  }

  const source = preview.source || "";
  const canRender = preview.kind === "markdown" || preview.kind === "html";
  const renderedModeLabel = preview.kind === "html" ? "页面预览" : "渲染预览";
  const htmlSource = buildHtmlPreviewSource(workspace, preview);
  const shouldFallbackToSource = preview.kind === "html" && mode === "rendered" && Boolean(preview.previewError);
  const kindLabel = preview.kind === "docx" ? "文本预览" : preview.kind === "image" ? "图片预览" : preview.kind;

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-gray-200 px-4 py-3">
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold text-gray-800">{preview.path}</div>
          <div className="text-xs text-gray-500">{kindLabel}</div>
        </div>
        <div className="flex items-center gap-2">
          {canRender && (
            <>
              <button
                type="button"
                className={`rounded-lg px-3 py-1 text-xs ${mode === "rendered" ? "bg-blue-100 text-blue-700" : "bg-gray-100 text-gray-600"}`}
                onClick={() => onModeChange("rendered")}
              >
                {renderedModeLabel}
              </button>
              <button
                type="button"
                className={`rounded-lg px-3 py-1 text-xs ${mode === "source" ? "bg-blue-100 text-blue-700" : "bg-gray-100 text-gray-600"}`}
                onClick={() => onModeChange("source")}
              >
                源码预览
              </button>
            </>
          )}
          <button
            type="button"
            className="rounded-lg bg-gray-100 px-3 py-1 text-xs text-gray-700 hover:bg-gray-200"
            onClick={() => onCopyPath?.(preview.path)}
          >
            复制路径
          </button>
          <button
            type="button"
            className="rounded-lg bg-gray-900 px-3 py-1 text-xs text-white hover:bg-gray-800"
            onClick={() => onOpenFile?.(preview.path)}
          >
            打开文件
          </button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto p-4">
        {preview.truncated && (
          <div className="mb-4 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-700">
            仅展示前 256 KB 内容
          </div>
        )}
        {preview.previewError && (
          <div className="mb-4 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-700">
            {preview.previewError}
          </div>
        )}
        {preview.kind === "image" && source ? (
          <div className="flex h-full min-h-[420px] items-center justify-center rounded-xl border border-gray-200 bg-gray-50 p-3">
            <img
              src={source}
              alt={preview.path}
              className="max-h-full max-w-full rounded-lg object-contain"
            />
          </div>
        ) : preview.kind === "image" ? (
          <div className="rounded-xl border border-dashed border-gray-300 bg-gray-50 p-4 text-sm text-gray-500">
            图片内容无法内嵌预览，请使用系统默认应用打开。
          </div>
        ) : preview.kind === "binary" ? (
          <div className="rounded-xl border border-dashed border-gray-300 bg-gray-50 p-4 text-sm text-gray-500">
            该文件暂不支持内嵌预览，请使用系统默认应用打开。
          </div>
        ) : preview.kind === "markdown" && mode === "rendered" ? (
          <div className="prose prose-sm max-w-none">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{source}</ReactMarkdown>
          </div>
        ) : preview.kind === "html" && mode === "rendered" && !shouldFallbackToSource ? (
          <iframe
            title={preview.path}
            srcDoc={htmlSource}
            sandbox="allow-same-origin"
            className="h-full min-h-[480px] w-full rounded-xl border border-gray-200 bg-white"
          />
        ) : preview.kind === "docx" ? (
          <div className="rounded-2xl border border-gray-200 bg-white p-4">
            <div className="mb-3 text-xs font-medium text-gray-500">文本预览</div>
            <pre className="whitespace-pre-wrap break-words text-sm leading-7 text-gray-700">{source || "未提取到可预览文本"}</pre>
          </div>
        ) : (
          <pre className="overflow-auto rounded-xl bg-gray-950 p-4 text-xs text-gray-100">{source}</pre>
        )}
        {preview.kind !== "binary" && preview.kind !== "docx" && preview.kind !== "image" && !source && (
          <div className="text-sm text-gray-400">文件内容为空</div>
        )}
      </div>
    </div>
  );
}
