import { redirect } from "next/navigation";

export default async function ServerSettingsPage() {
    redirect("/resources/pools");
}
