'use client'

import React, { createContext, useContext } from "react";
import { getMessages, Locale, Messages, translate, TranslateParams } from "./core";
export type { Locale, Messages, TranslateParams } from "./core";

type I18nContextValue = {
  locale: Locale;
  messages: Messages;
  t: (key: string, params?: TranslateParams) => string;
};

const I18nContext = createContext<I18nContextValue>({
  locale: "en",
  messages: getMessages("en"),
  t: (key, params) => translate(getMessages("en"), key, params),
});

export function I18nProvider({
  locale,
  messages,
  children,
}: {
  locale: Locale;
  messages: Messages;
  children: React.ReactNode;
}) {
  const value = React.useMemo<I18nContextValue>(() => ({
    locale,
    messages,
    t: (key, params) => translate(messages, key, params),
  }), [locale, messages]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  return useContext(I18nContext);
}

export function useT() {
  return useI18n().t;
}
