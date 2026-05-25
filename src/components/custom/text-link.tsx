import { ExternalLink } from "lucide-react";
import Link from "next/link";

export default function TextLink({ href, children }: { href: string; children: React.ReactNode }) {
    return (
        <Link href={href} target="_blank">
            <div className="flex gap-1 items-center pt-1">
                <p className="text-blue-500 hover:underline flex-1">{children}</p>
                <div className="text-blue-500 hover:underline"> <ExternalLink size={15} /></div>
            </div>
        </Link>
    )
}