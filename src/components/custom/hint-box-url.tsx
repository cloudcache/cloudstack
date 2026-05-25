import Link from "next/link";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "../ui/tooltip";
import { Button } from "../ui/button";
import { QuestionMarkIcon } from "@radix-ui/react-icons";




export function HintBoxUrl({ url }: { url: string }) {

    const uri = new URL(url);


    return <TooltipProvider>
        <Tooltip>
            <TooltipTrigger asChild>
                <Link href={url} target="_blank">
                    <Button type="button" variant="outline" className="h-8 w-8 p-0"><QuestionMarkIcon /></Button>
                </Link>
            </TooltipTrigger>
            <TooltipContent>
                <p>Absprung zu {uri.hostname}</p>
            </TooltipContent>
        </Tooltip>
    </TooltipProvider>
}