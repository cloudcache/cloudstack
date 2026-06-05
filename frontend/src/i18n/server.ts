import { cookies, headers } from "next/headers";
import enMessages from "./messages/en.json";
import zhMessages from "./messages/zh.json";

type Locale = "en" | "zh";
type Messages = Record<string, string>;

const dictionaries: Record<Locale, Messages> = {
  en: enMessages,
  zh: zhMessages,
};

function normalizeServerLocale(value: string | undefined | null): Locale {
  if (!value) return "en";
  return value.toLowerCase().startsWith("zh") ? "zh" : "en";
}

function getServerMessages(locale: Locale): Messages {
  return dictionaries[locale];
}

function translateServer(
  messages: Messages,
  key: string,
  params?: Record<string, string | number | null | undefined>,
): string {
  const template = messages[key] ?? dictionaries.en[key] ?? key;
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (_, name) => String(params[name] ?? ""));
}

export async function getLocale() {
  const cookieStore = await cookies();
  const cookieLocale = cookieStore.get("qs_locale")?.value;
  if (cookieLocale) return normalizeServerLocale(cookieLocale);

  const headerStore = await headers();
  return normalizeServerLocale(headerStore.get("accept-language"));
}

export async function getT() {
  const locale = await getLocale();
  const messages = getServerMessages(locale);
  return {
    locale,
    messages,
    t: (key: string, params?: Record<string, string | number | null | undefined>) =>
      translateServer(messages, key, params),
  };
}
