import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { SkillLibraryView } from "../SkillLibraryView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

class MockIntersectionObserver {
  static instances: MockIntersectionObserver[] = [];

  callback: IntersectionObserverCallback;

  constructor(callback: IntersectionObserverCallback) {
    this.callback = callback;
    MockIntersectionObserver.instances.push(this);
  }

  observe() {}
  unobserve() {}
  disconnect() {}

  triggerIntersect() {
    this.callback([{ isIntersecting: true } as IntersectionObserverEntry], this as unknown as IntersectionObserver);
  }
}

describe("SkillLibraryView library behavior", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    MockIntersectionObserver.instances = [];
    Object.defineProperty(window, "IntersectionObserver", {
      writable: true,
      value: MockIntersectionObserver,
    });
  });

  test("appends later pages without reshuffling already rendered cards", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "list_clawhub_library") {
        if (!payload?.cursor) {
          return Promise.resolve({
            items: [
              { slug: "alpha", name: "Alpha", summary: "Alpha summary", tags: ["one"], stars: 10, downloads: 100 },
              { slug: "beta", name: "Beta", summary: "Beta summary", tags: ["two"], stars: 9, downloads: 90 },
            ],
            next_cursor: "cursor-2",
            last_synced_at: "2026-03-21T10:00:00.000Z",
          });
        }
        return Promise.resolve({
          items: [
            { slug: "gamma", name: "Gamma", summary: "Gamma summary", tags: ["three"], stars: 1, downloads: 999 },
          ],
          next_cursor: null,
          last_synced_at: "2026-03-21T10:00:00.000Z",
        });
      }
      return Promise.resolve(null);
    });

    render(<SkillLibraryView installedSkillIds={new Set<string>()} onInstall={async () => {}} />);

    await waitFor(() => {
      expect(screen.getByText("Alpha")).toBeInTheDocument();
      expect(screen.getByText("Beta")).toBeInTheDocument();
    });

    MockIntersectionObserver.instances.at(-1)?.triggerIntersect();

    await waitFor(() => {
      expect(screen.getByText("Gamma")).toBeInTheDocument();
    });

    const alpha = screen.getByText("Alpha");
    const gamma = screen.getByText("Gamma");
    expect(alpha.compareDocumentPosition(gamma) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  test("shows last sync timestamp from the backend response", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_clawhub_library") {
        return Promise.resolve({
          items: [
            { slug: "alpha", name: "Alpha", summary: "Alpha summary", tags: ["one"], stars: 10, downloads: 100 },
          ],
          next_cursor: null,
          last_synced_at: "2026-03-21T10:00:00.000Z",
        });
      }
      return Promise.resolve(null);
    });

    render(<SkillLibraryView installedSkillIds={new Set<string>()} onInstall={async () => {}} />);

    await waitFor(() => {
      expect(screen.getByText(/最近同步/i)).toBeInTheDocument();
    });
  });

  test("allows manually refreshing the skill library", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_clawhub_library") {
        return Promise.resolve({
          items: [
            { slug: "alpha", name: "Alpha", summary: "Alpha summary", tags: ["one"], stars: 10, downloads: 100 },
          ],
          next_cursor: null,
          last_synced_at: "2026-03-21T10:00:00.000Z",
        });
      }
      if (command === "sync_skillhub_catalog") {
        return Promise.resolve({
          total_skills: 1,
          last_synced_at: "2026-03-21T12:00:00.000Z",
          refreshed: true,
        });
      }
      return Promise.resolve(null);
    });

    render(<SkillLibraryView installedSkillIds={new Set<string>()} onInstall={async () => {}} />);

    await waitFor(() => {
      expect(screen.getByText("Alpha")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "刷新技能库" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("sync_skillhub_catalog", { force: true });
    });
  });
});
