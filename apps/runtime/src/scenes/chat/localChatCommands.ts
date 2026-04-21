export type ClawhubInstallCommand = {
  query: string;
};

const CLAWHUB_INSTALL_PREFIX =
  /^(?:(?:请)?(?:帮我)?(?:安装|安装skill|安装技能|帮我安装skill|帮我安装技能|安装 skill|安装 技能)[:：]?\s*)?clawhub\s+install\s+(.+)$/i;

export function parseClawhubInstallCommand(text: string): ClawhubInstallCommand | null {
  const match = text.trim().match(CLAWHUB_INSTALL_PREFIX);
  const query = match?.[1]?.trim();
  if (!query) {
    return null;
  }
  return { query };
}

export function normalizeClawhubCommandLookupKey(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}
