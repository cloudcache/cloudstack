'use client'

import { Button } from "@/components/ui/button";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useRouter, useSearchParams, usePathname } from "next/navigation";

interface PaginationControlsProps {
    page: number;
    total: number;
    perPage: number;
    /** The URL search param name that controls this paginator, e.g. "tx_page" */
    param: string;
}

export default function PaginationControls({ page, total, perPage, param }: PaginationControlsProps) {
    const router = useRouter();
    const pathname = usePathname();
    const searchParams = useSearchParams();
    const totalPages = Math.max(1, Math.ceil(total / perPage));

    const navigate = (newPage: number) => {
        const params = new URLSearchParams(searchParams.toString());
        params.set(param, String(newPage));
        router.push(`${pathname}?${params.toString()}`);
    };

    if (totalPages <= 1) return null;

    return (
        <div className="flex items-center justify-between pt-3 border-t">
            <span className="text-xs text-muted-foreground">
                Page {page} of {totalPages} &nbsp;·&nbsp; {total} total
            </span>
            <div className="flex gap-1">
                <Button
                    variant="outline" size="sm"
                    disabled={page <= 1}
                    onClick={() => navigate(page - 1)}
                >
                    <ChevronLeft className="h-4 w-4" />
                </Button>
                <Button
                    variant="outline" size="sm"
                    disabled={page >= totalPages}
                    onClick={() => navigate(page + 1)}
                >
                    <ChevronRight className="h-4 w-4" />
                </Button>
            </div>
        </div>
    );
}
