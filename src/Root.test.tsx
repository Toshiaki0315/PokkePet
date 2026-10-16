import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./App", () => ({ default: () => <div>main popup</div> }));
vi.mock("./Settings", () => ({ default: () => <div>settings dialog</div> }));

import Root from "./Root";

describe("Root", () => {
  it("renders the settings dialog in the settings window", () => {
    render(<Root label="settings" />);
    expect(screen.getByText("settings dialog")).toBeInTheDocument();
  });

  it("renders the popup in every other window", () => {
    render(<Root label="main" />);
    expect(screen.getByText("main popup")).toBeInTheDocument();
  });
});
