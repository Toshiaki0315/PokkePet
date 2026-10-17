import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings as SettingsModel } from "./api";
import { DEFAULT_SETTINGS, PRESETS } from "./defaults";

const api = vi.hoisted(() => ({
  getState: vi.fn(),
  setName: vi.fn(),
  onStateChanged: vi.fn(),
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
  onSettingsChanged: vi.fn(),
}));
vi.mock("./api", () => api);

import Settings from "./Settings";

const custom: SettingsModel = { showGauges: false, notifyCalls: true, sleepStartHour: 22, sleepEndHour: 6, bodyColor: "#112233", buttonColor: "#445566" };
let emit: (s: SettingsModel) => void;
let emitState: (v: { name: string | null }) => void;
const unlisten = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  api.getState.mockResolvedValue({ name: "ごま" });
  api.setName.mockImplementation((name: string) => Promise.resolve({ name }));
  api.onStateChanged.mockImplementation((h: (v: { name: string | null }) => void) => {
    emitState = h;
    return Promise.resolve(unlisten);
  });
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
    await waitFor(() => expect(screen.getByRole("checkbox", { name: "ゲージを表示" })).not.toBeChecked());
    expect(screen.getByRole("checkbox", { name: "ゲージを表示" })).not.toBeChecked();
    expect(colorInput("本体の色").value).toBe("#112233");
    expect(colorInput("ボタンの色").value).toBe("#445566");
  });

  it("starts from the default settings until the saved ones arrive", () => {
    api.getSettings.mockReturnValue(new Promise(() => {}));
    render(<Settings />);
    expect(screen.getByRole("checkbox", { name: "ゲージを表示" })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: "お世話の呼びかけを通知" })).toBeChecked(); // 既定はオン
    expect(colorInput("本体の色").value).toBe(DEFAULT_SETTINGS.bodyColor);
  });

  it("has no switch for the stage number any more", async () => {
    render(<Settings />);
    await screen.findByRole("checkbox", { name: "ゲージを表示" });
    expect(screen.queryByRole("checkbox", { name: "段階を表示" })).toBeNull();
  });

  it("saves the gauge switch", async () => {
    render(<Settings />);
    await userEvent.click(await screen.findByRole("checkbox", { name: "ゲージを表示" }));
    expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, showGauges: true });
  });

  it("shows and saves the call-notification switch", async () => {
    render(<Settings />);
    const box = await screen.findByRole("checkbox", { name: "お世話の呼びかけを通知" });
    expect(box).toBeChecked();
    await userEvent.click(box);
    expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, notifyCalls: false });
  });

  it("saves a new body colour and a new button colour", async () => {
    render(<Settings />);
    await screen.findByRole("checkbox", { name: "ゲージを表示" });
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
    await screen.findByRole("checkbox", { name: "ゲージを表示" });
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

  describe("pet name", () => {
    it("shows the current name", async () => {
      render(<Settings />);
      await waitFor(() => expect(screen.getByRole("textbox", { name: "名前" })).toHaveValue("ごま"));
    });

    it("renames the pet", async () => {
      render(<Settings />);
      const box = screen.getByRole("textbox", { name: "名前" });
      await waitFor(() => expect(box).toHaveValue("ごま"));
      await userEvent.clear(box);
      await userEvent.type(box, "もち");
      await userEvent.click(screen.getByRole("button", { name: "名前を変更" }));
      expect(api.setName).toHaveBeenCalledWith("もち");
    });

    it("works before the pet has a name", async () => {
      api.getState.mockResolvedValue({ name: null });
      render(<Settings />);
      const box = screen.getByRole("textbox", { name: "名前" });
      await waitFor(() => expect(api.getState).toHaveBeenCalled());
      expect(box).toHaveValue("");
      expect(screen.getByRole("button", { name: "名前を変更" })).toBeDisabled();
    });

    it("shows an error when the name is rejected", async () => {
      api.setName.mockRejectedValue(new Error("too long"));
      render(<Settings />);
      await waitFor(() => expect(screen.getByRole("textbox", { name: "名前" })).toHaveValue("ごま"));
      await userEvent.type(screen.getByRole("textbox", { name: "名前" }), "x");
      await userEvent.click(screen.getByRole("button", { name: "名前を変更" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("too long");
    });

    it("follows the name when it changes in the popup", async () => {
      render(<Settings />);
      await waitFor(() => expect(screen.getByRole("textbox", { name: "名前" })).toHaveValue("ごま"));
      await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
      emitState({ name: "ぽっけ" });
      await waitFor(() => expect(screen.getByRole("textbox", { name: "名前" })).toHaveValue("ぽっけ"));
      emitState({ name: null });
      await waitFor(() => expect(screen.getByRole("textbox", { name: "名前" })).toHaveValue(""));
    });

    it("shows an error when the current name cannot be loaded", async () => {
      api.getState.mockRejectedValue(new Error("state unreadable"));
      render(<Settings />);
      expect(await screen.findByRole("alert")).toHaveTextContent("state unreadable");
    });
  });

  describe("sleep hours", () => {
    const select = (name: string) => screen.getByRole("combobox", { name }) as HTMLSelectElement;

    it("shows the saved sleep hours", async () => {
      render(<Settings />);
      await waitFor(() => expect(select("おやすみ開始").value).toBe("22"));
      expect(select("おやすみ終了").value).toBe("6");
    });

    it("offers every hour of the day", async () => {
      render(<Settings />);
      await screen.findByRole("checkbox", { name: "ゲージを表示" });
      const options = Array.from(select("おやすみ開始").options).map((o) => o.textContent);
      expect(options).toHaveLength(24);
      expect(options[0]).toBe("0時");
      expect(options[23]).toBe("23時");
    });

    it("saves a new start hour as a number", async () => {
      render(<Settings />);
      await waitFor(() => expect(select("おやすみ開始").value).toBe("22"));
      await userEvent.selectOptions(select("おやすみ開始"), "21");
      expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, sleepStartHour: 21 });
    });

    it("saves a new end hour as a number", async () => {
      render(<Settings />);
      await waitFor(() => expect(select("おやすみ終了").value).toBe("6"));
      await userEvent.selectOptions(select("おやすみ終了"), "8");
      expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, sleepEndHour: 8 });
    });

    it("explains that equal hours mean no sleeping", async () => {
      api.getSettings.mockResolvedValue({ ...custom, sleepStartHour: 0, sleepEndHour: 0 });
      render(<Settings />);
      expect(await screen.findByText("開始と終了が同じ時刻なので、おやすみしません")).toBeInTheDocument();
    });

    it("does not show that note for a normal window", async () => {
      render(<Settings />);
      await waitFor(() => expect(select("おやすみ開始").value).toBe("22"));
      expect(screen.queryByText("開始と終了が同じ時刻なので、おやすみしません")).toBeNull();
    });

    it("describes the current window", async () => {
      render(<Settings />);
      expect(await screen.findByText("22時〜翌6時")).toBeInTheDocument();
    });

    it("describes a window within one day without 翌", async () => {
      api.getSettings.mockResolvedValue({ ...custom, sleepStartHour: 13, sleepEndHour: 15 });
      render(<Settings />);
      expect(await screen.findByText("13時〜15時")).toBeInTheDocument();
    });
  });
});
