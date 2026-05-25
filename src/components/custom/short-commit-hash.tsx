import React from 'react';
import { Code } from './code';

export default function ShortCommitHash({ children }: { children?: string }) {
    const shortHash = children ? children.slice(0, 7) : '';
    if (!shortHash) {
        return <></>;
    }
    return (<Code copieableValue={children}>{shortHash}</Code>);
};

