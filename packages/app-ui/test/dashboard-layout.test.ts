import { describe, expect, it } from "vitest";
import { dashboardColumns } from "../src/dashboard-layout";

const WIDE = 3440 / 1440;
const TV = 1920 / 1080;

describe("dashboardColumns", () => {
  it("puts up to four groups in one row on a 21:9 screen", () => {
    expect([1, 2, 3, 4].map((n) => dashboardColumns(n, WIDE))).toEqual([1, 2, 3, 4]);
  });
  it("balances rows instead of leaving one orphan tile", () => {
    expect(dashboardColumns(5, WIDE)).toBe(3); // 3+2, not 4+1
    expect(dashboardColumns(7, WIDE)).toBe(4); // 4+3
    expect(dashboardColumns(4, TV)).toBe(2); // 2+2, not 3+1
  });
  it("caps columns at three on 16:9 and two on narrow windows", () => {
    expect(dashboardColumns(9, TV)).toBe(3);
    expect(dashboardColumns(9, 1.2)).toBe(2);
  });
  it("returns one column for an empty list", () => {
    expect(dashboardColumns(0, WIDE)).toBe(1);
  });
});
