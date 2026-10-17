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

const sound = vi.hoisted(() => ({ play: vi.fn(), useSound: vi.fn() }));
vi.mock("./sound", () => ({
  useSound: (enabled: boolean, tuning: unknown) => {
    sound.useSound(enabled, tuning);
    return sound.play;
  },
}));

import Settings from "./Settings";

const custom: SettingsModel = { showGauges: false, notifyCalls: true, soundEnabled: true, soundVolume: 50, soundPitch: 0, launchAtLogin: false, strictMode: false, companionEnabled: true, breakReminder: true, autoSleep: true, sleepStartHour: 22, sleepEndHour: 6, bodyColor: "#112233", buttonColor: "#445566" };
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

  it("shows and saves the sound switch", async () => {
    render(<Settings />);
    const box = await screen.findByRole("checkbox", { name: "鳴き声を鳴らす" });
    expect(box).toBeChecked();
    await userEvent.click(box);
    expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, soundEnabled: false });
  });

  it("saves the cry volume and pitch", async () => {
    render(<Settings />);
    const volume = await screen.findByRole("slider", { name: "鳴き声の大きさ" });
    fireEvent.change(volume, { target: { value: "80" } });
    expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, soundVolume: 80 });
    fireEvent.change(screen.getByRole("slider", { name: "鳴き声の高さ" }), { target: { value: "-1" } });
    expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, soundPitch: -1 });
  });

  it("lets you listen to a cry with the current volume and pitch", async () => {
    api.getSettings.mockResolvedValue({ ...custom, soundVolume: 70, soundPitch: 1 });
    render(<Settings />);
    await waitFor(() => expect(sound.useSound).toHaveBeenLastCalledWith(true, { volume: 70, pitch: 1 }));
    await userEvent.click(screen.getByRole("button", { name: "ためしに鳴かせる" }));
    expect(sound.play).toHaveBeenCalledWith("happy", { personality: "calm", stage: 5 });
  });

  it("disables the sound tuning while the sound is off", async () => {
    api.getSettings.mockResolvedValue({ ...custom, soundEnabled: false });
    render(<Settings />);
    await waitFor(() => expect(screen.getByRole("slider", { name: "鳴き声の大きさ" })).toBeDisabled());
    expect(screen.getByRole("slider", { name: "鳴き声の高さ" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "ためしに鳴かせる" })).toBeDisabled();
  });

  it.each([
    ["作業に寄り添う", "companionEnabled"],
    ["休憩の声かけ", "breakReminder"],
    ["おやすみ時間を自動で合わせる", "autoSleep"],
  ] as const)("shows and saves the %s switch", async (name, key) => {
    render(<Settings />);
    const box = await screen.findByRole("checkbox", { name });
    expect(box).toBeChecked();
    await userEvent.click(box);
    expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, [key]: false });
  });

  it("disables the companion sub-options while the companion is off", async () => {
    api.getSettings.mockResolvedValue({ ...custom, companionEnabled: false });
    render(<Settings />);
    await waitFor(() => expect(screen.getByRole("checkbox", { name: "作業に寄り添う" })).not.toBeChecked());
    expect(screen.getByRole("checkbox", { name: "休憩の声かけ" })).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: "おやすみ時間を自動で合わせる" })).toBeDisabled();
  });

  it("shows and saves the strict-mode switch", async () => {
    render(<Settings />);
    const box = await screen.findByRole("checkbox", { name: "きびしいモード" });
    expect(box).not.toBeChecked();
    await userEvent.click(box);
    expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, strictMode: true });
  });

  it("shows and saves the launch-at-login switch", async () => {
    render(<Settings />);
    const box = await screen.findByRole("checkbox", { name: "ログイン時に自動で起動" });
    expect(box).not.toBeChecked();
    await userEvent.click(box);
    expect(api.saveSettings).toHaveBeenCalledWith({ ...custom, launchAtLogin: true });
  });

  it("starts with the sound on until the saved settings arrive", () => {
    api.getSettings.mockReturnValue(new Promise(() => {}));
    render(<Settings />);
    expect(screen.getByRole("checkbox", { name: "鳴き声を鳴らす" })).toBeChecked();
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

  it("no longer has a pet-name field (the name is decided at hatching and fixed until the pet leaves)", async () => {
    render(<Settings />);
    await screen.findByRole("checkbox", { name: "ゲージを表示" });
    expect(screen.queryByRole("textbox", { name: "名前" })).toBeNull();
    expect(screen.queryByRole("button", { name: "名前を変更" })).toBeNull();
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
