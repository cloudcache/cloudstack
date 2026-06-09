import { redirect } from "next/navigation";
import AppShell from "../app-shell";
import { getUserSession } from "@/server/utils/action-wrapper.utils";
import PodsStatusPollingProvider from "@/components/custom/pods-status-polling-provider";

// User console tier — requires an authenticated session.
export default async function ConsoleLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const session = await getUserSession();
  if (!session) redirect("/auth");

  return (
    <>
      <AppShell tier="console">{children}</AppShell>
      <PodsStatusPollingProvider />
    </>
  );
}
