'use client'

import { ArrowLeft } from "lucide-react";
import { Button } from "../ui/button";

export default function NavigateBackButton() {
    return <Button variant="ghost" onClick={() => window.history.back()}><ArrowLeft /></Button>;
}