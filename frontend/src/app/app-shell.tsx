import { cookies } from "next/headers";
import { Suspense } from "react";
import { SidebarProvider } from "@/components/ui/sidebar";
import { AppSidebar } from "./sidebar";
import { SidebarTier } from "./sidebar-client";
import { BreadcrumbsGenerator } from "@/components/custom/breadcrumbs-generator";
import FullLoadingSpinner from "@/components/ui/full-loading-spinnter";

/** Authenticated app shell (sidebar + breadcrumbs + content). Used by the
 *  console and admin route groups; the portal group renders without it. */
export default async function AppShell({
  children,
  tier,
}: {
  children: React.ReactNode;
  tier: SidebarTier;
}) {
  const cookieStore = await cookies();
  const defaultOpen = (cookieStore.get("sidebar:state")?.value ?? "true") === "true";

  return (
    <SidebarProvider defaultOpen={defaultOpen}>
      <AppSidebar tier={tier} />
      <main className="flex w-full flex-col items-center">
        <div className="w-full max-w-8xl px-2 lg:px-4">
          <div className="flex-col md:flex p-6">
            <BreadcrumbsGenerator />
            <Suspense fallback={<FullLoadingSpinner />}>{children}</Suspense>
          </div>
        </div>
      </main>
    </SidebarProvider>
  );
}
