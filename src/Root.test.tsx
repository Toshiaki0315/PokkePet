import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./App", () => ({ default: () => <div>main popup</div> }));
vi.mock("./Settings", () => ({ default: () => <div>settings dialog</div> }));
vi.mock("./Dex", () => ({ default: () => <div>dex window</div> }));
vi.mock("./Diary", () => ({ default: () => <div>diary window</div> }));
vi.mock("./EggCode", () => ({ default: () => <div>egg code window</div> }));
vi.mock("./Walker", () => ({ default: () => <div>walker window</div> }));
vi.mock("./Souvenirs", () => ({ default: () => <div>souvenirs window</div> }));

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

  it("renders the souvenir collection in the souvenirs window", () => {
    render(<Root label="souvenirs" />);
    expect(screen.getByText("souvenirs window")).toBeInTheDocument();
  });

  it("renders the egg code exchange in the eggcode window", () => {
    render(<Root label="eggcode" />);
    expect(screen.getByText("egg code window")).toBeInTheDocument();
  });

  it("renders the walking pet in the walker window", () => {
    render(<Root label="walker" />);
    expect(screen.getByText("walker window")).toBeInTheDocument();
  });

  it("renders the popup in every other window", () => {
    render(<Root label="main" />);
    expect(screen.getByText("main popup")).toBeInTheDocument();
  });
});
