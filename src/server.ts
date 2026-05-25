import { createServer } from 'http'
import { parse } from 'url'
import next from 'next'
import { FancyConsoleUtils } from './shared/utils/fancy-console.utils'

// Source: https://nextjs.org/docs/app/building-your-application/configuring/custom-server

const port = parseInt(process.env.PORT || '3000', 10)
const dev = process.env.NODE_ENV !== 'production'

async function initializeNextJs() {

    FancyConsoleUtils.printQuickStack();

    const app = next({ dev });
    const handle = app.getRequestHandler();

    app.prepare().then(() => {

        const server = createServer((req, res) => {
            const parsedUrl = parse(req.url!, true)
            handle(req, res, parsedUrl)
        });

        server.listen(port)

        console.log(
            `> Server listening at http://localhost:${port} as ${dev ? 'development' : process.env.NODE_ENV
            }`
        )
    });
}

initializeNextJs();
