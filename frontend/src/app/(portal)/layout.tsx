import { LanguageMenuButton } from "@/components/i18n/language-switcher";

// Public portal tier (landing + auth + error pages) — no app shell, no auth gate.
export default function PortalLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="min-h-screen">
      <div className="flex justify-end p-4">
        <LanguageMenuButton />
      </div>
      {children}
    </div>
  );
}
