import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings as SettingsModel } from "./api";
import { DEFAULT_SETTINGS, PRESETS } from "./defaults";

const api = vi.hoisted(() => ({
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
  onSettingsChanged: vi.fn(),
}));
vi.mock("./api", () => api);

import Settings from "./Settings";

const custom: SettingsModel = { showStage: true, showGauges: false, bodyColor: "#112233", buttonColor: "#445566" };
let emit: (s: SettingsModel) => void;
const unlisten = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  api.getSettings.mockResolvedValue(custom);
  api.saveSettings.mockImplementation((s: SettingsModel) => Promise.resolve(s));
  api.onSettingsChanged.mockImplementation((h: (s: SettingsModel) => void) => {
    emit = h;
    return Promise.resolve(unlisten);
  });
});

const colorInput = (name: string) => screen.getByLabelText(name) as HTMLInputElement;

describe("Settings dialog", () => {
  it("shows the saved settings", async () => {
    render(<Settings />);
    await waitFor(() => expect(screen.getByRole("checkbox", { name: "段階を表示" })).toBeChecked());
    expect(screen.getByRole("checkbox", { name: "ゲージを表示" })).not.toBeChecked();
    expect(colorInput("本体の色").value).toBe("#112233");
    expect(colorInput("ボタンの色").value).toBe("#445566");
  });

  it("starts from the default settings until the saved ones arrive", () => {
    api.getSettings.mockReturnValue(new Promise(() => {}));
    render(<Settings />);
    expect(screen.getByRole("checkbox", { name: "段階を表示" })).not.toBeChecked();
    expect(colorInput("本体の色").value).toBe(DEFAULT_SETTINGS.bodyColor);
  });

  it("saves the stage switch", async () => {
    render(<Settings />);
    await userEvent.click(await screen.findByRole("checkbox", { name: "段階を表示" }));
    expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, showStage: false });
  });

  it("saves the gauge switch", async () => {
    render(<Settings />);
    await userEvent.click(await screen.findByRole("checkbox", { name: "ゲージを表示" }));
    expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, showGauges: true });
  });

  it("saves a new body colour and a new button colour", async () => {
    render(<Settings />);
    await screen.findByRole("checkbox", { name: "段階を表示" });
    fireEvent.change(colorInput("本体の色"), { target: { value: "#aabbcc" } });
    await waitFor(() => expect(api.saveSettings).toHaveBeenLastCalledWith({ ...custom, bodyColor: "#aabbcc" }));
    fireEvent.change(colorInput("ボタンの色"), { target: { value: "#ddeeff" } });
    await waitFor(() => expect(api.saveSettings).toHaveBeenLastCalledWith({ ...custom, bodyColor: "#aabbcc", buttonColor: "#ddeeff" }));
  });

  it.each(PRESETS)("applies the $name preset", async ({ name, bodyColor, buttonColor }) => {
    render(<Settings />);
    await userEvent.click(await screen.findByRole("button", { name }));
    expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, bodyColor, buttonColor });
  });

  it("resets everything to the defaults", async () => {
    render(<Settings />);
    await userEvent.click(await screen.findByRole("button", { name: "初期設定に戻す" }));
    expect(api.saveSettings).toHaveBeenCalledWith(DEFAULT_SETTINGS);
  });

  it("shows an error when saving fails and keeps the previous values", async () => {
    api.saveSettings.mockRejectedValue(new Error("disk full"));
    render(<Settings />);
    await userEvent.click(await screen.findByRole("checkbox", { name: "ゲージを表示" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    expect(screen.getByRole("checkbox", { name: "ゲージを表示" })).not.toBeChecked();
  });

  it("clears the error after a successful save", async () => {
    api.saveSettings.mockRejectedValueOnce("boom").mockImplementation((s: SettingsModel) => Promise.resolve(s));
    render(<Settings />);
    await userEvent.click(await screen.findByRole("checkbox", { name: "ゲージを表示" }));
    await screen.findByRole("alert");
    await userEvent.click(screen.getByRole("checkbox", { name: "ゲージを表示" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("shows an error when loading the settings fails", async () => {
    api.getSettings.mockRejectedValue(new Error("unreadable"));
    render(<Settings />);
    expect(await screen.findByRole("alert")).toHaveTextContent("unreadable");
  });

  it("follows settings-changed events (e.g. the hidden command in the main window)", async () => {
    render(<Settings />);
    await screen.findByRole("checkbox", { name: "段階を表示" });
    await waitFor(() => expect(api.onSettingsChanged).toHaveBeenCalled());
    emit({ ...custom, showGauges: true });
    await waitFor(() => expect(screen.getByRole("checkbox", { name: "ゲージを表示" })).toBeChecked());
  });

  it("unsubscribes on unmount, even if unmounted before listen resolves", async () => {
    let resolve!: (f: () => void) => void;
    api.onSettingsChanged.mockReturnValue(new Promise((r) => (resolve = r)));
    const { unmount } = render(<Settings />);
    unmount();
    resolve(unlisten);
    await waitFor(() => expect(unlisten).toHaveBeenCalled());
  });

  it("offers presets that really differ from each other", () => {
    expect(new Set(PRESETS.map((p) => p.bodyColor)).size).toBe(PRESETS.length);
    for (const p of PRESETS) {
      expect(p.bodyColor).toMatch(/^#[0-9a-f]{6}$/);
      expect(p.buttonColor).toMatch(/^#[0-9a-f]{6}$/);
    }
    expect(PRESETS[0]).toMatchObject({ bodyColor: DEFAULT_SETTINGS.bodyColor, buttonColor: DEFAULT_SETTINGS.buttonColor });
  });
});
