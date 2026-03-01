import { create } from "zustand";

export type SettingsTab =
  | "account"
  | "settings"
  | "personalization"
  | "about";

export interface ToastMessage {
  id: string;
  type: "success" | "error" | "warning" | "info";
  title: string;
  body?: string;
}

interface UIStore {
  /** 设置弹窗是否打开 */
  settingsOpen: boolean;
  /** 当前选中的设置 tab */
  settingsTab: SettingsTab;
  /** Toast 通知队列 */
  toasts: ToastMessage[];

  openSettings: (tab?: SettingsTab) => void;
  closeSettings: () => void;
  showToast: (msg: Omit<ToastMessage, "id">) => void;
  dismissToast: (id: string) => void;
}

export const useUIStore = create<UIStore>((set) => ({
  settingsOpen: false,
  settingsTab: "settings",
  toasts: [],

  openSettings: (tab) =>
    set({ settingsOpen: true, settingsTab: tab ?? "settings" }),

  closeSettings: () => set({ settingsOpen: false }),

  showToast: (msg) =>
    set((state) => ({
      toasts: [...state.toasts, { ...msg, id: crypto.randomUUID() }],
    })),

  dismissToast: (id) =>
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    })),
}));
