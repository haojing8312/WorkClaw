import type { EmployeeHubOpenRequest } from "./EmployeeHubScene";

type EmployeeHubTab = EmployeeHubOpenRequest["tab"];

export function createEmployeeHubOpenRequest(
  tab: EmployeeHubTab = "overview",
  options?: {
    highlightEmployeeId?: string | null;
    highlightEmployeeName?: string | null;
  },
): EmployeeHubOpenRequest {
  return {
    nonce: Date.now(),
    tab,
    highlightEmployeeId: options?.highlightEmployeeId ?? null,
    highlightEmployeeName: options?.highlightEmployeeName ?? null,
  };
}

export function retargetEmployeeHubOpenRequest(
  previous: EmployeeHubOpenRequest | null | undefined,
  tab: EmployeeHubTab = "overview",
): EmployeeHubOpenRequest {
  return createEmployeeHubOpenRequest(tab, {
    highlightEmployeeId: previous?.highlightEmployeeId ?? null,
    highlightEmployeeName: previous?.highlightEmployeeName ?? null,
  });
}
