import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./App", () => ({ default: () => <div>main popup</div> }));
vi.mock("./Settings", () => ({ default: () => <div>settings dialog</div> }));
vi.mock("./Dex", () => ({ default: () => <div>dex window</div> }));
vi.mock("./Diary", () => ({ default: () => <div>diary window</div> }));

import Root from "./Root";

describe("Root", () => {
  it("renders the settings dialog in the settings window", () => {
    render(<Root label="settings" />);
    expect(screen.getByText("settings dialog")).toBeInTheDocument();
  });

  it("renders the dex in the dex window", () => {
    render(<Root label="dex" />);
    expect(screen.getByText("dex window")).toBeInTheDocument();
  });

  it("renders the diary in the diary window", () => {
    render(<Root label="diary" />);
    expect(screen.getByText("diary window")).toBeInTheDocument();
  });

  it("renders the popup in every other window", () => {
    render(<Root label="main" />);
    expect(screen.getByText("main popup")).toBeInTheDocument();
  });
});
