# Firestore URL Opener

A Rust application that retrieves URLs from Firestore and automatically opens them in a web browser.

## Features

- Fetch URLs from Firestore
- Automatically open retrieved URLs in a browser

## Usage

1. Create a `.env` file in the project root and set up environment variables:

   For instructions on obtaining the credentials.json file, please refer to:
   [Initialize the SDK in non-Google environments](https://firebase.google.com/docs/admin/setup#initialize_the_sdk_in_non-google_environments)

   ```
   # Google Cloud credentials
   GOOGLE_APPLICATION_CREDENTIALS=path/to/your/credentials.json

   # Firebase project settings
   PROJECT_ID=your-firebase-project-id 
   ```

2. Run the application:
   ```bash
   cargo run
   ```
