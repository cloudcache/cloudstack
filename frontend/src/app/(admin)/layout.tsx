import { redirect } from "next/navigation";
import AppShell from "../app-shell";
import { getUserSession } from "@/server/utils/action-wrapper.utils";
import { UserGroupUtils } from "@/shared/utils/role.utils";

// Admin tier — requires a global-admin session.
export default async function AdminLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const session = await getUserSession();
  if (!session) redirect("/auth");
  if (!UserGroupUtils.isAdmin(session)) redirect("/unauthorized");

  return <AppShell tier="admin">{children}</AppShell>;
}
