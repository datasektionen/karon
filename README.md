# Karon

VoteIT middleware for SSO to handle attendence in chapter meetings (SM).

## Usage

Built to work in conjonction with [Kerberos](https://github.com/datasektionen/kerberos/) and [VoteIT](https://voteit.se/), to allow for seamless NFC identification to check in or out of chapter meetings.

> [!IMPORTANT]
> Karon will does not inherently allow for creating or managing meetings - these have to be manually hard-coded to be created and managed when compiling and running the program. The [`compose.yml`](./compose.yml) is also missing card uids, which have to be manually filled in when testing.

## Configuration

### Environment

| Setting | Required | Format |
| --- | --- | --- |
| DATABASE_URL | Yes | `postgresql://USER:PWD@HOST:PORT/DB` |
| HIVE_API_URL | Yes | `https://hive.datasektionen.se/api/v1` |
| HIVE_API_KEY | Yes | `secret` |
| VOTEIT_URL | Yes | `https://voteit.se` |
| SSO_URL | Yes | `https://sso.datasektionen.se` |

## API

| Endpoint | Method | Input | Output |
| --- | --- | --- | --- |
| `/card` | `POST` | A bearer token tied to a specific meeting (called *kerberos token*) and an RFID uid for the person checking in. | KTH-id of person checking in. |
| `/card/onboard` | `POST` | A bearer token tied to a specific meeting (called *kerberos token*) and `<kth-id>#<rfid-uid>`. | KTH-id of person. |
| `/onboard` | `POST` | A bearer token generated for onboarding members to SSO (called *onboard token*) and `<kth-id>#<rfid-uid>`. | - |
