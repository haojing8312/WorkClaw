import { describe, expect, test } from "vitest";
import { SEARCH_PROVIDER_CATALOG } from "../search-provider-catalog";
import { SEARCH_PRESETS } from "../lib/search-config";

describe("search provider ordering", () => {
  test("keeps metaso first in search presets", () => {
    expect(SEARCH_PRESETS[1]?.value).toBe("metaso");
  });

  test("keeps metaso first in quick setup catalog", () => {
    expect(SEARCH_PROVIDER_CATALOG[0]?.id).toBe("metaso");
  });
});
