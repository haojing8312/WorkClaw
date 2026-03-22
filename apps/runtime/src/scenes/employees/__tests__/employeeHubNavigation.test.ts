import { describe, expect, it, vi } from "vitest";
import {
  createEmployeeHubOpenRequest,
  retargetEmployeeHubOpenRequest,
} from "../employeeHubNavigation";

describe("employeeHubNavigation", () => {
  it("creates employee-hub open requests with highlight payload", () => {
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(12345);

    expect(
      createEmployeeHubOpenRequest("employees", {
        highlightEmployeeId: "emp-1",
        highlightEmployeeName: "Alpha",
      }),
    ).toEqual({
      nonce: 12345,
      tab: "employees",
      highlightEmployeeId: "emp-1",
      highlightEmployeeName: "Alpha",
    });
    nowSpy.mockRestore();
  });

  it("retargets employee-hub requests without dropping highlight state", () => {
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(23456);

    expect(
      retargetEmployeeHubOpenRequest(
        {
          nonce: 1,
          tab: "employees",
          highlightEmployeeId: "emp-2",
          highlightEmployeeName: "Beta",
        },
        "overview",
      ),
    ).toEqual({
      nonce: 23456,
      tab: "overview",
      highlightEmployeeId: "emp-2",
      highlightEmployeeName: "Beta",
    });
    nowSpy.mockRestore();
  });
});
