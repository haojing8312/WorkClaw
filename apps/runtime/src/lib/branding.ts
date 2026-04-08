import BRANDING from "../branding.generated";
import brandLogoSrc from "../assets/branding/current/app-logo.png";

export { BRANDING, brandLogoSrc };

export function storageKey(name: string): string {
  return `${BRANDING.localStoragePrefix}:${name}`;
}

export function brandDataRootDirName(): string {
  return `.${BRANDING.localStoragePrefix}`;
}

export function brandDefaultWorkspacePathExample(usernamePlaceholder = "<用户名>"): string {
  return `C:\\Users\\${usernamePlaceholder}\\${brandDataRootDirName()}\\workspace`;
}
