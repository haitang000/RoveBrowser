import { english } from "./locales/en";

export const INTERFACE_LANGUAGES = [
  ["zh-CN", "简体中文"],
  ["en", "English"],
] as const;
export type InterfaceLanguage = (typeof INTERFACE_LANGUAGES)[number][0];
export const DEFAULT_INTERFACE_LANGUAGE: InterfaceLanguage = "zh-CN";
export const LANGUAGE_STORAGE_KEY = "rove:interface-language";

export function interfaceLanguage(value: unknown): InterfaceLanguage {
  return INTERFACE_LANGUAGES.find(([id]) => id === value)?.[0] ?? DEFAULT_INTERFACE_LANGUAGE;
}

/** Chinese is the source locale. Named placeholders let other locales reorder values. */
export function translate(language: InterfaceLanguage, source: string, values?: Record<string, string | number>): string {
  const message = language === "en" && Object.hasOwn(english, source) ? english[source]! : source;
  return values ? message.replace(/\{(\w+)\}/g, (token, key: string) =>
    Object.hasOwn(values, key) ? String(values[key]) : token,
  ) : message;
}
