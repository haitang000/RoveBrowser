import { english } from "./locales/en";
import { traditionalChinese } from "./locales/zh-TW";

export const INTERFACE_LANGUAGES = [
  ["zh-CN", "简体中文"],
  ["zh-TW", "繁體中文"],
  ["en", "English"],
] as const;
export type InterfaceLanguage = (typeof INTERFACE_LANGUAGES)[number][0];
export const DEFAULT_INTERFACE_LANGUAGE: InterfaceLanguage = "zh-CN";
export const LANGUAGE_STORAGE_KEY = "rove:interface-language";

export function interfaceLanguage(value: unknown): InterfaceLanguage {
  return INTERFACE_LANGUAGES.find(([id]) => id === value)?.[0] ?? DEFAULT_INTERFACE_LANGUAGE;
}

/** Simplified Chinese is the source locale. Named placeholders let other locales reorder values. */
const CATALOGS: Partial<Record<InterfaceLanguage, Readonly<Record<string, string>>>> = {
  en: english,
  "zh-TW": traditionalChinese,
};

export function translate(language: InterfaceLanguage, source: string, values?: Record<string, string | number>): string {
  const catalog = CATALOGS[language];
  const message = catalog && Object.hasOwn(catalog, source) ? catalog[source]! : source;
  return values ? message.replace(/\{(\w+)\}/g, (token, key: string) =>
    Object.hasOwn(values, key) ? String(values[key]) : token,
  ) : message;
}
