import { QuestionMarkCircledIcon } from "@radix-ui/react-icons";
import { FormLabel } from "../ui/form";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "../ui/tooltip";


export default function FormLabelWithQuestion(
    { children, hint }: { children: React.ReactNode, hint: string | React.ReactNode }
) {
    return <div className="flex gap-1 mt-1 pb-1">
        <FormLabel>{children}</FormLabel>
        <div>
            <TooltipProvider>
                <Tooltip>
                    <TooltipTrigger asChild><QuestionMarkCircledIcon /></TooltipTrigger>
                    <TooltipContent>
                        <p className="max-w-[350px]">{hint}</p>
                    </TooltipContent>
                </Tooltip>
            </TooltipProvider>
        </div>
    </div>
}