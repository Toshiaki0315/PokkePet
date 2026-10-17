import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  getEggCode: vi.fn(),
  receiveEgg: vi.fn(),
}));
vi.mock("./api", () => api);

import EggCode from "./EggCode";

const writeText = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  api.getEggCode.mockResolvedValue("ABCDE-FGHJK");
  api.receiveEgg.mockResolvedValue({});
  writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
});

describe("Egg code window", () => {
  it("shows my own code", async () => {
    render(<EggCode />);
    expect(await screen.findByText("ABCDE-FGHJK")).toBeInTheDocument();
  });

  it("copies my code to the clipboard", async () => {
    render(<EggCode />);
    await screen.findByText("ABCDE-FGHJK");
    await userEvent.click(screen.getByRole("button", { name: "コピー" }));
    expect(writeText).toHaveBeenCalledWith("ABCDE-FGHJK");
    expect(await screen.findByRole("button", { name: "コピーしました" })).toBeInTheDocument();
  });

  it("explains when copying is not possible", async () => {
    writeText.mockRejectedValue(new Error("denied"));
    render(<EggCode />);
    await screen.findByText("ABCDE-FGHJK");
    await userEvent.click(screen.getByRole("button", { name: "コピー" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("コピーできませんでした");
  });

  it("cannot copy before the code has arrived", () => {
    api.getEggCode.mockReturnValue(new Promise(() => {}));
    render(<EggCode />);
    expect(screen.getByRole("button", { name: "コピー" })).toBeDisabled();
  });

  it("shows an error when the code cannot be fetched", async () => {
    api.getEggCode.mockRejectedValue(new Error("unreadable"));
    render(<EggCode />);
    expect(await screen.findByRole("alert")).toHaveTextContent("unreadable");
  });

  it("asks before welcoming a friend's egg, and does nothing if the player backs out", async () => {
    render(<EggCode />);
    await screen.findByText("ABCDE-FGHJK");
    const welcome = screen.getByRole("button", { name: "迎える" });
    expect(welcome).toBeDisabled();
    await userEvent.type(screen.getByRole("textbox", { name: "友だちのコード" }), "XYZ23-45678");
    await userEvent.click(screen.getByRole("button", { name: "迎える" }));
    const dialog = await screen.findByRole("alertdialog", { name: "確認" });
    expect(dialog).toHaveTextContent("お別れ");
    await userEvent.click(screen.getByRole("button", { name: "やめる" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(api.receiveEgg).not.toHaveBeenCalled();
  });

  it("welcomes the egg once confirmed and shows the new code", async () => {
    render(<EggCode />);
    await screen.findByText("ABCDE-FGHJK");
    await userEvent.type(screen.getByRole("textbox", { name: "友だちのコード" }), "XYZ23-45678");
    await userEvent.click(screen.getByRole("button", { name: "迎える" }));
    api.getEggCode.mockResolvedValue("NEW22-33445");
    const dialog = await screen.findByRole("alertdialog", { name: "確認" });
    await userEvent.click(within(dialog).getByRole("button", { name: "迎える" }));
    expect(api.receiveEgg).toHaveBeenCalledWith("XYZ23-45678");
    expect(await screen.findByText(/新しいたまごが届きました/)).toBeInTheDocument();
    expect(await screen.findByText("NEW22-33445")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "友だちのコード" })).toHaveValue("");
  });

  it("shows why a wrong code is refused and keeps the pet", async () => {
    api.receiveEgg.mockRejectedValue(new Error("the egg code does not match (a typo?)"));
    render(<EggCode />);
    await screen.findByText("ABCDE-FGHJK");
    await userEvent.type(screen.getByRole("textbox", { name: "友だちのコード" }), "XYZ23-45679");
    await userEvent.click(screen.getByRole("button", { name: "迎える" }));
    await userEvent.click(within(await screen.findByRole("alertdialog", { name: "確認" })).getByRole("button", { name: "迎える" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("a typo?");
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(screen.queryByText(/新しいたまごが届きました/)).toBeNull();
  });

  it("shows an error when the new code cannot be fetched after welcoming", async () => {
    render(<EggCode />);
    await screen.findByText("ABCDE-FGHJK");
    await userEvent.type(screen.getByRole("textbox", { name: "友だちのコード" }), "XYZ23-45678");
    await userEvent.click(screen.getByRole("button", { name: "迎える" }));
    api.getEggCode.mockRejectedValue(new Error("gone"));
    await userEvent.click(within(await screen.findByRole("alertdialog", { name: "確認" })).getByRole("button", { name: "迎える" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("gone");
  });

  it("hides the welcome message again when the player starts typing another code", async () => {
    render(<EggCode />);
    await screen.findByText("ABCDE-FGHJK");
    const box = screen.getByRole("textbox", { name: "友だちのコード" });
    await userEvent.type(box, "XYZ23-45678");
    await userEvent.click(screen.getByRole("button", { name: "迎える" }));
    await userEvent.click(within(await screen.findByRole("alertdialog", { name: "確認" })).getByRole("button", { name: "迎える" }));
    await screen.findByText(/新しいたまごが届きました/);
    await userEvent.type(box, "A");
    expect(screen.queryByText(/新しいたまごが届きました/)).toBeNull();
  });
});
