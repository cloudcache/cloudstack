import enMessages from "./messages/en.json";
import zhMessages from "./messages/zh.json";

export const locales = ["en", "zh"] as const;
export type Locale = typeof locales[number];
export type Messages = Record<string, string>;
export type TranslateParams = Record<string, string | number | null | undefined>;

const dictionaries: Record<Locale, Messages> = {
  en: enMessages,
  zh: zhMessages,
};

export function normalizeLocale(value: string | undefined | null): Locale {
  if (!value) return "en";
  const locale = value.toLowerCase();
  return locale.startsWith("zh") ? "zh" : "en";
}

export function translate(messages: Messages, key: string, params?: TranslateParams): string {
  const template = messages[key] ?? dictionaries.en[key] ?? key;
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (_, name) => String(params[name] ?? ""));
}

export function getMessages(locale: Locale): Messages {
  return dictionaries[locale];
}
