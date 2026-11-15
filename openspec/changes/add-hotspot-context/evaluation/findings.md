# Hotspot evaluation findings (task 2.5)

Every finding with hotspot context in the second run (tests not ranked),
sorted by hand (see design.md, "Evaluation"). CleanArchitecture and
eShop have none: no file changed in 5 commits of a window.

## server

| commit | rule | file | severity | commits | complexity | category |
|---|---|---|---|---|---|---|
| 26057c62 | complexity/cyclomatic-new | `util/Seeder/Pipeline/RecipeBuilderExtensions.cs:33` | warning | 15 | 18 | support code |
| 2a690361 | complexity/cyclomatic-increase | `util/Seeder/Factories/UserSeeder.cs:15` | error | 5 | 14 | support code |
| 242894a0 | complexity/cyclomatic-increase | `src/Core/Vault/Services/Implementations/CipherService.cs:748` | warning | 8 | 216 | central code |
| ebefe07a | tests/unchanged-test | `src/Admin/AdminConsole/Controllers/OrganizationsController.cs:764` | warning | 16 | 178 | central code |
| ebefe07a | complexity/cyclomatic-increase | `src/Core/Dirt/Services/Implementations/EventService.cs:304` | warning | 13 | 134 | central code |
| 04a4d754 | complexity/cyclomatic-increase | `src/Infrastructure.EntityFramework/AdminConsole/Repositories/OrganizationUserRepository.cs:896` | warning | 14 | 168 | central code |
| c22f7be6 | complexity/cyclomatic-new | `bitwarden_license/src/Sso/Utilities/Saml2/Saml2OptionsExtensions.cs:35` | error | 5 | 42 | small file |
| c22f7be6 | complexity/cyclomatic-new | `bitwarden_license/src/Sso/Utilities/Saml2/Saml2OptionsExtensions.cs:140` | warning | 5 | 42 | small file |
| 8dbb66d8 | duplication/duplicate-block | `bitwarden_license/src/Sso/Controllers/AccountController.cs:60` | warning | 10 | 145 | central code |
| 8dbb66d8 | complexity/cyclomatic-increase | `bitwarden_license/src/Sso/Controllers/AccountController.cs:585` | error | 10 | 145 | central code |
| 8dbb66d8 | complexity/cyclomatic-increase | `bitwarden_license/src/Sso/Controllers/AccountController.cs:873` | warning | 10 | 145 | central code |
| 8dbb66d8 | complexity/cyclomatic-increase | `src/Core/AdminConsole/OrganizationFeatures/InviteLinks/AcceptOrganizationInviteLinkCommand.cs:71` | error | 11 | 28 | small file |
| 8dbb66d8 | complexity/cyclomatic-increase | `src/Core/AdminConsole/OrganizationFeatures/InviteLinks/ConfirmOrganizationInviteLinkCommand.cs:120` | error | 7 | 31 | small file |
| 8dbb66d8 | duplication/duplicate-block | `src/Core/AdminConsole/OrganizationFeatures/InviteLinks/ConfirmOrganizationInviteLinkCommand.cs:123` | warning | 7 | 31 | small file |
| 8dbb66d8 | duplication/duplicate-block | `src/Core/AdminConsole/OrganizationFeatures/OrganizationUsers/RestoreUser/v1/RestoreOrganizationUserCommand.cs:31` | warning | 8 | 94 | central code |
| 8dbb66d8 | duplication/duplicate-block | `src/Core/AdminConsole/OrganizationFeatures/OrganizationUsers/RestoreUser/v1/RestoreOrganizationUserCommand.cs:34` | warning | 8 | 94 | central code |
| 8dbb66d8 | complexity/cyclomatic-new | `src/Core/AdminConsole/OrganizationFeatures/OrganizationUsers/RestoreUser/v1/RestoreOrganizationUserCommand.cs:91` | error | 8 | 94 | central code |
| 8dbb66d8 | complexity/cyclomatic-increase | `src/Core/AdminConsole/Services/Implementations/OrganizationService.cs:524` | warning | 14 | 245 | central code |
| 8dbb66d8 | duplication/duplicate-block | `src/Core/AdminConsole/Services/Implementations/OrganizationService.cs:866` | warning | 14 | 245 | central code |
| 8dbb66d8 | duplication/duplicate-block | `src/Core/OrganizationFeatures/OrganizationSubscriptions/UpdateSecretsManagerSubscriptionCommand.cs:27` | warning | 6 | 88 | central code |
| 3185b44e | complexity/cyclomatic-increase | `src/Admin/AdminConsole/Controllers/OrganizationsController.cs:434` | warning | 15 | 178 | central code |
| 3185b44e | duplication/duplicate-block | `src/Core/AdminConsole/OrganizationFeatures/OrganizationUsers/RestoreUser/v1/RestoreOrganizationUserCommand.cs:27` | warning | 7 | 81 | central code |
| 3185b44e | duplication/duplicate-block | `src/Core/AdminConsole/OrganizationFeatures/OrganizationUsers/RestoreUser/v1/RestoreOrganizationUserCommand.cs:28` | warning | 7 | 81 | central code |
| 3185b44e | complexity/cyclomatic-increase | `src/Core/AdminConsole/OrganizationFeatures/OrganizationUsers/RestoreUser/v1/RestoreOrganizationUserCommand.cs:84` | warning | 7 | 81 | central code |
| 3185b44e | complexity/cyclomatic-increase | `src/Core/AdminConsole/OrganizationFeatures/OrganizationUsers/RestoreUser/v1/RestoreOrganizationUserCommand.cs:219` | warning | 7 | 81 | central code |
| 4822c428 | duplication/duplicate-block | `src/Api/Vault/Controllers/CiphersController.cs:72` | warning | 18 | 325 | central code |
| 4822c428 | duplication/duplicate-block | `src/Api/Vault/Controllers/CiphersController.cs:74` | warning | 18 | 325 | central code |
| 4822c428 | duplication/duplicate-block | `src/Api/Vault/Controllers/CiphersController.cs:78` | warning | 18 | 325 | central code |
| 4822c428 | duplication/duplicate-block | `src/Api/Vault/Controllers/CiphersController.cs:79` | warning | 18 | 325 | central code |
| 4822c428 | duplication/duplicate-block | `src/Api/Vault/Controllers/CiphersController.cs:81` | warning | 18 | 325 | central code |

## clients

| commit | rule | file | severity | commits | complexity | category |
|---|---|---|---|---|---|---|
| df1c77bb | tests/unchanged-test | `apps/desktop/src/vault/app/vault-v3/vault.component.ts:619` | warning | 39 | 235 | central code |
| a8237e9a | tests/unchanged-test | `apps/browser/src/autofill/content/autofill-init.ts:54` | warning | 5 | 70 | central code |
| a8237e9a | complexity/cyclomatic-increase | `apps/browser/src/autofill/services/collect-autofill-content.service.ts:285` | error | 20 | 387 | central code |
| ea55f7b9 | complexity/cyclomatic-increase | `apps/desktop/src/autofill/services/desktop-fido2-user-interface.service.ts:628` | warning | 11 | 125 | central code |
| ea55f7b9 | complexity/cyclomatic-increase | `apps/desktop/src/autofill/services/desktop-fido2-user-interface.service.ts:649` | warning | 11 | 125 | central code |
| 473a21d3 | complexity/cyclomatic-increase | `libs/vault/src/vault-item-dialog/vault-item-dialog.component.ts:414` | warning | 13 | 173 | central code |
| 473a21d3 | complexity/cyclomatic-increase | `libs/vault/src/vault-item-dialog/vault-item-dialog.component.ts:751` | warning | 13 | 173 | central code |
| 473a21d3 | complexity/cyclomatic-new | `libs/vault/src/vault-item-dialog/vault-item-dialog.component.ts:985` | warning | 13 | 173 | central code |
| 09e5296b | tests/unchanged-test | `apps/web/src/app/admin-console/organizations/policies/policy-edit-definitions/organization-data-ownership.component.ts:131` | warning | 8 | 21 | small file |
| 09e5296b | tests/unchanged-test | `libs/importer/src/services/import.service.ts:678` | warning | 14 | 172 | central code |
| 09e5296b | tests/unchanged-test | `libs/vault/src/components/vault-items-table/vault-items-table.component.ts:601` | warning | 29 | 210 | central code |
| 80507e69 | tests/unchanged-test | `apps/desktop/src/app/accounts/settings-dialog.component.ts:848` | warning | 21 | 93 | central code |
| 80507e69 | complexity/cyclomatic-increase | `apps/web/src/app/admin-console/organizations/policies/policy-drawer-story.helper.ts:165` | warning | 9 | 19 | support code |
| 7cb2858c | tests/unchanged-test | `bitwarden_license/bit-web/src/app/pam/access-rules/access-rules.component.ts:133` | warning | 7 | 57 | central code |
| 5f44fb5a | tests/unchanged-test | `apps/web/src/app/tools/send/send-access/send-view.component.ts:68` | warning | 8 | 24 | small file |
| 4d76bbfb | tests/unchanged-test | `apps/browser/src/background/main.background.ts:629` | warning | 92 | 102 | composition root |
| 4d76bbfb | tests/unchanged-test | `apps/cli/src/service-container/service-container.ts:404` | warning | 65 | 24 | composition root |
| 484911b2 | tests/unchanged-test | `apps/desktop/src/main/native-messaging.main.ts:488` | warning | 16 | 126 | central code |
| 6b8ce547 | tests/unchanged-test | `apps/browser/src/background/main.background.ts:2323` | warning | 91 | 102 | composition root |
| 6b8ce547 | complexity/cyclomatic-increase | `apps/browser/src/background/runtime.background.ts:165` | warning | 27 | 129 | central code |
| 65663899 | complexity/cyclomatic-new | `apps/web/src/app/billing/organizations/organization-subscription-cloud.component.ts:348` | warning | 9 | 234 | central code |
| d8660243 | complexity/cyclomatic-increase | `apps/browser/src/background/main.background.ts:628` | warning | 90 | 102 | composition root |
| d8660243 | tests/unchanged-test | `apps/browser/src/background/main.background.ts:628` | warning | 90 | 102 | composition root |
| 88fa0962 | tests/unchanged-test | `apps/browser/src/background/main.background.ts:628` | warning | 89 | 99 | composition root |
| 88fa0962 | tests/unchanged-test | `apps/cli/src/service-container/service-container.ts:400` | warning | 64 | 24 | composition root |
| 88fa0962 | duplication/duplicate-block | `libs/common/src/vault/services/cipher.service.ts:83` | warning | 34 | 322 | central code |

## Totals

| repository | central code | composition root | small file | support code |
|---|---|---|---|---|
| server | 23 | 0 | 5 | 2 |
| clients | 16 | 7 | 2 | 1 |
