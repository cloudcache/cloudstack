import type { Metadata } from "next";
import { Inter } from "next/font/google";
import { cn } from "@/frontend/utils/utils"
import { Toaster } from "@/components/ui/sonner"
import "./globals.css";
import { ConfirmDialog } from "@/components/custom/confirm-dialog";
import { InputDialog } from "@/components/custom/input-dialog";
import { getT } from "@/i18n/server";
import { I18nProvider } from "@/i18n";

const inter = Inter({
  subsets: ["latin"],
  variable: "--font-sans",
});

export const metadata: Metadata = {
  title: "QuickStack",
  description: "", // todo
  icons: [
    { rel: "favicon", url: "/quickstack-icon-dark.png" },
    { rel: "icon", url: "/quickstack-icon-dark.png" },
    { rel: "apple-touch-icon", url: "/quickstack-icon-dark.png" }
  ],
};

export default async function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const { locale, messages } = await getT();

  return (
    <html lang={locale}>
      <body className={cn(
        "min-h-screen bg-background font-sans antialiased",
        inter.variable
      )}>
        <I18nProvider locale={locale} messages={messages}>
          {/* Per-tier shells live in the (portal) / (admin) / (console) route groups. */}
          {children}
          <Toaster />
          <ConfirmDialog />
          <InputDialog />
        </I18nProvider>
      </body>
    </html>
  );
}
