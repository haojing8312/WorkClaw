import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FilePreviewPane, WorkspaceFilePreview } from "./FilePreviewPane";

type WorkspaceFileItem = {
  path: string;
  name: string;
  size: number;
  kind: string;
};

type FileTreeNode = {
  path: string;
  name: string;
  kind: "directory" | "file";
  size: number;
  entry?: WorkspaceFileItem;
  children: FileTreeNode[];
};

interface WorkspaceFilesPanelProps {
  workspace: string;
  touchedFiles: string[];
  active: boolean;
}

function splitFileName(name: string): { baseName: string; extension: string } {
  const lastDot = name.lastIndexOf(".");
  if (lastDot <= 0 || lastDot === name.length - 1) {
    return { baseName: name, extension: "" };
  }

  return {
    baseName: name.slice(0, lastDot),
    extension: name.slice(lastDot),
  };
}

function fileKindBadgeLabel(entry: WorkspaceFileItem | undefined, name: string): string {
  const fallback = name.split(".").pop()?.toUpperCase();
  switch (entry?.kind) {
    case "markdown":
      return "MD";
    case "html":
      return "HTML";
    case "docx":
      return "DOCX";
    case "text":
      return fallback || "TEXT";
    default:
      return fallback || (entry?.kind || "FILE").toUpperCase();
  }
}

function joinWorkspacePath(workspace: string, relativePath: string): string {
  const separator = workspace.includes("\\") ? "\\" : "/";
  const normalizedWorkspace = workspace.replace(/[\\/]+$/, "");
  const normalizedRelative = relativePath.replace(/[\\/]+/g, separator);
  return `${normalizedWorkspace}${separator}${normalizedRelative}`;
}

function buildTree(files: WorkspaceFileItem[]): FileTreeNode[] {
  const root: FileTreeNode = {
    path: "",
    name: "",
    kind: "directory",
    size: 0,
    children: [],
  };
  const nodeMap = new Map<string, FileTreeNode>([["", root]]);

  const sortedFiles = [...files].sort((left, right) => left.path.localeCompare(right.path));
  for (const file of sortedFiles) {
    const segments = file.path.split("/").filter(Boolean);
    let currentPath = "";
    let parent = root;

    for (let index = 0; index < segments.length; index += 1) {
      const segment = segments[index];
      currentPath = currentPath ? `${currentPath}/${segment}` : segment;
      const isLeaf = index === segments.length - 1;
      const kind = isLeaf && file.kind !== "directory" ? "file" : "directory";
      let node = nodeMap.get(currentPath);
      if (!node) {
        node = {
          path: currentPath,
          name: segment,
          kind,
          size: isLeaf ? file.size : 0,
          entry: isLeaf ? file : undefined,
          children: [],
        };
        nodeMap.set(currentPath, node);
        parent.children.push(node);
      } else if (isLeaf) {
        node.kind = kind;
        node.size = file.size;
        node.entry = file;
      }
      parent = node;
    }
  }

  const sortChildren = (node: FileTreeNode) => {
    node.children.sort((left, right) => {
      if (left.kind !== right.kind) {
        return left.kind === "directory" ? -1 : 1;
      }
      return left.name.localeCompare(right.name);
    });
    node.children.forEach(sortChildren);
  };

  sortChildren(root);
  return root.children;
}

function ancestorDirectories(path: string): string[] {
  const segments = path.split("/").filter(Boolean);
  return segments.slice(0, -1).map((_, index) => segments.slice(0, index + 1).join("/"));
}

function renderTreeNodes({
  nodes,
  level,
  expandedDirs,
  selectedPath,
  touchedFiles,
  onToggleDirectory,
  onSelectFile,
}: {
  nodes: FileTreeNode[];
  level: number;
  expandedDirs: Set<string>;
  selectedPath: string;
  touchedFiles: string[];
  onToggleDirectory: (path: string) => void;
  onSelectFile: (path: string) => void;
}) {
  return nodes.map((node) => {
    const touched = touchedFiles.includes(node.path);
    const paddingLeft = 12 + level * 16;
    const isExpanded = node.kind === "directory" && expandedDirs.has(node.path);

    if (node.kind === "directory") {
      return (
        <div key={node.path}>
          <button
            type="button"
            aria-label={node.name}
            onClick={() => onToggleDirectory(node.path)}
            className="flex w-full items-center gap-2 rounded-xl px-3 py-2 text-left hover:bg-gray-50"
            style={{ paddingLeft }}
          >
            <span className="text-xs text-gray-400">{isExpanded ? "▾" : "▸"}</span>
            <span className="truncate text-sm font-medium text-gray-700">{node.name}</span>
          </button>
          {isExpanded &&
            renderTreeNodes({
              nodes: node.children,
              level: level + 1,
              expandedDirs,
              selectedPath,
              touchedFiles,
              onToggleDirectory,
              onSelectFile,
            })}
        </div>
      );
    }

    return (
      <button
        key={node.path}
        type="button"
        aria-label={node.name}
        title={node.path}
        onClick={() => onSelectFile(node.path)}
        className={`flex w-full items-center justify-between rounded-xl px-3 py-2 text-left ${
          selectedPath === node.path ? "bg-blue-50" : "hover:bg-gray-50"
        }`}
        style={{ paddingLeft }}
      >
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <div className="flex min-w-0 items-baseline">
              <span className="truncate text-sm text-gray-900">{splitFileName(node.name).baseName}</span>
              {splitFileName(node.name).extension && (
                <span className="shrink-0 text-sm text-gray-500">{splitFileName(node.name).extension}</span>
              )}
            </div>
            <span className="shrink-0 rounded-full bg-gray-100 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-gray-600">
              {fileKindBadgeLabel(node.entry, node.name)}
            </span>
          </div>
          {touched && <div className="mt-1 text-[11px] text-blue-600">本轮生成</div>}
        </div>
        <div className="ml-3 text-xs text-gray-400">{Math.round((node.size || 0) / 102.4) / 10} KB</div>
      </button>
    );
  });
}

export function WorkspaceFilesPanel({ workspace, touchedFiles, active }: WorkspaceFilesPanelProps) {
  const [files, setFiles] = useState<WorkspaceFileItem[]>([]);
  const [selectedPath, setSelectedPath] = useState("");
  const [preview, setPreview] = useState<WorkspaceFilePreview | null>(null);
  const [search, setSearch] = useState("");
  const [previewMode, setPreviewMode] = useState<"rendered" | "source">("rendered");
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!active || !workspace) return;
    let cancelled = false;
    invoke<WorkspaceFileItem[]>("list_workspace_files", { workspace })
      .then((result) => {
        if (cancelled) return;
        const nextFiles = result || [];
        setFiles(nextFiles);
        setExpandedDirs(new Set(touchedFiles.flatMap(ancestorDirectories)));
        setSelectedPath((current) => current || touchedFiles[0] || nextFiles.find((item) => item.kind !== "directory")?.path || "");
      })
      .catch(() => {
        if (!cancelled) {
          setFiles([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [active, workspace, touchedFiles]);

  useEffect(() => {
    if (!selectedPath || !workspace) {
      setPreview(null);
      return;
    }
    let cancelled = false;
    invoke<WorkspaceFilePreview>("read_workspace_file_preview", {
      workspace,
      relativePath: selectedPath,
    })
      .then((result) => {
        if (!cancelled) {
          setPreview(result);
          setPreviewMode(result?.kind === "html" ? "rendered" : result?.kind === "markdown" ? "rendered" : "source");
        }
      })
      .catch(() => {
        if (!cancelled) {
          setPreview(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [selectedPath, workspace]);

  const filteredFiles = useMemo(() => {
    const query = search.trim().toLowerCase();
    return files.filter((file) => !query || file.path.toLowerCase().includes(query) || file.name.toLowerCase().includes(query));
  }, [files, search]);

  const tree = useMemo(() => buildTree(filteredFiles), [filteredFiles]);

  useEffect(() => {
    if (!selectedPath) return;
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      ancestorDirectories(selectedPath).forEach((path) => next.add(path));
      return next;
    });
  }, [selectedPath]);

  return (
    <div className="flex h-full min-h-[640px] overflow-hidden rounded-2xl border border-gray-200 bg-white">
      <div className="flex w-[300px] min-w-[280px] max-w-[320px] shrink-0 flex-col border-r border-gray-200">
        <div className="flex items-center justify-between px-4 py-4">
          <div className="text-2xl font-semibold text-gray-900">文件</div>
          <button
            type="button"
            className="rounded-lg p-2 text-gray-500 hover:bg-gray-100"
            onClick={() => {
              if (workspace) {
                void invoke("open_external_url", { url: workspace });
              }
            }}
            aria-label="打开工作空间"
          >
            ↗
          </button>
        </div>
        <div className="px-4 pb-4">
          <input
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="搜索文件..."
            className="w-full rounded-2xl border border-gray-200 px-4 py-3 text-sm outline-none focus:border-blue-300"
          />
        </div>
        <div className="min-h-0 flex-1 overflow-auto px-3 pb-3">
          <div className="space-y-1">
            {tree.length === 0 ? (
              <div className="px-3 py-8 text-sm text-gray-400">当前工作空间没有可展示的文件</div>
            ) : (
              renderTreeNodes({
                nodes: tree,
                level: 0,
                expandedDirs,
                selectedPath,
                touchedFiles,
                onToggleDirectory: (path) =>
                  setExpandedDirs((prev) => {
                    const next = new Set(prev);
                    if (next.has(path)) {
                      next.delete(path);
                    } else {
                      next.add(path);
                    }
                    return next;
                  }),
                onSelectFile: (path) => setSelectedPath(path),
              })
            )}
          </div>
        </div>
      </div>
      <div className="min-w-0 flex-1">
        <FilePreviewPane
          preview={preview}
          workspace={workspace}
          mode={previewMode}
          onModeChange={setPreviewMode}
          onCopyPath={(path) => {
            void globalThis.navigator?.clipboard?.writeText?.(path);
          }}
          onOpenFile={(path) => {
            if (workspace) {
              void invoke("open_external_url", { url: joinWorkspacePath(workspace, path) });
            }
          }}
        />
      </div>
    </div>
  );
}
