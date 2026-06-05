'use client'

import { Languages } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useI18n } from "@/i18n";

export function LanguageSwitcher() {
  const { locale, t } = useI18n();

  const switchLocale = (nextLocale: "en" | "zh") => {
    document.cookie = `qs_locale=${nextLocale}; path=/; max-age=31536000; SameSite=Lax`;
    window.location.reload();
  };

  return (
    <>
      <DropdownMenuItem disabled>
        <Languages />
        <span>{t("language.label")}: {locale === "zh" ? t("language.zh") : t("language.en")}</span>
      </DropdownMenuItem>
      <DropdownMenuItem onClick={() => switchLocale("en")}>
        <span>{t("language.en")}</span>
      </DropdownMenuItem>
      <DropdownMenuItem onClick={() => switchLocale("zh")}>
        <span>{t("language.zh")}</span>
      </DropdownMenuItem>
    </>
  );
}

export function LanguageMenuButton({ compact = false }: { compact?: boolean }) {
  const { locale, t } = useI18n();

  const switchLocale = (nextLocale: "en" | "zh") => {
    document.cookie = `qs_locale=${nextLocale}; path=/; max-age=31536000; SameSite=Lax`;
    window.location.reload();
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size={compact ? "icon" : "sm"} className={compact ? undefined : "w-full justify-start gap-2"}>
          <Languages className="h-4 w-4" />
          {!compact && <span>{locale === "zh" ? t("language.zh") : t("language.en")}</span>}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onClick={() => switchLocale("en")}>
          {t("language.en")}
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => switchLocale("zh")}>
          {t("language.zh")}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
