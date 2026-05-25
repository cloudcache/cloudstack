const fs = require('fs');
const path = require('path');

// Define the directory to search in
const DIRECTORY = './src/shared/model/generated-zod/';

// Define the string to search for and the replacement string
const SEARCH_STRING = 'import * as imports from "../../../../prisma/null"';
const REPLACEMENT_STRING = '';

// Function to recursively find all .ts files in the directory
function findFiles(dir, ext, files = []) {
    const items = fs.readdirSync(dir);
    for (const item of items) {
        const fullPath = path.join(dir, item);
        const stat = fs.statSync(fullPath);
        if (stat.isDirectory()) {
            findFiles(fullPath, ext, files);
        } else if (path.extname(fullPath) === ext) {
            files.push(fullPath);
        }
    }
    return files;
}

// Function to replace content in the files
function replaceInFile(filePath, searchString, replacementString) {
    const data = fs.readFileSync(filePath, 'utf8');
    const result = data.replace(searchString, replacementString);
    fs.writeFileSync(filePath, result, 'utf8');
}

// Find all .ts files in the directory
const tsFiles = findFiles(DIRECTORY, '.ts');

// Replace the specified content in each file
tsFiles.forEach((file) => {
    replaceInFile(file, SEARCH_STRING, REPLACEMENT_STRING);
});

console.log('Replacement complete.');
