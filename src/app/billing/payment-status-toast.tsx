'use client'

import { useEffect } from "react";
import { useSearchParams } from "next/navigation";
import { toast } from "sonner";

export default function PaymentStatusToast() {
    const searchParams = useSearchParams();
    const payment = searchParams.get('payment');

    useEffect(() => {
        if (payment === 'success') {
            toast.success('Payment successful! Your wallet will be credited shortly.');
        } else if (payment === 'cancelled') {
            toast.info('Payment cancelled.');
        }
    }, [payment]);

    return null;
}
