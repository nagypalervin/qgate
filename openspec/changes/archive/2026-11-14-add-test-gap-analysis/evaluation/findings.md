# Evaluation findings (task 2.6)

Every `tests/` finding of the first run, sorted by hand (see design.md,
"Evaluation"). "After" says whether the run with the three decisions
still reports it.

## clients

| commit | rule | file | category | after |
|---|---|---|---|---|
| df1c77bb | unchanged-test | `apps/desktop/src/vault/app/vault-v3/vault.component.ts:619` | should | yes |
| a8237e9a | unchanged-test | `apps/browser/src/autofill/content/autofill-init.ts:54` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/access-tokens/sm-access-token.utils.ts:10` | should | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/models/sm-access-policy.response.ts:15` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/models/sm-access-token.response.ts:15` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/models/sm-bulk-status.response.ts:11` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/models/sm-counts.response.ts:16` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/models/sm-event.response.ts:19` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/models/sm-export.response.ts:14` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/models/sm-grantee.response.ts:9` | should | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/models/sm-machine-account.response.ts:15` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/models/sm-project.response.ts:16` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/models/sm-secret-version.response.ts:13` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/models/sm-secret.response.ts:8` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/secrets/sm-secret-project-names.ts:10` | should | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/secrets/sm-secret-request.ts:17` | should | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/spec/sm-access-test-utils.ts:25` | test code | no |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/spec/sm-test-utils.ts:18` | test code | no |
| 95f58974 | missing-test | `bitwarden_license/bit-cli/src/secrets-manager/trash/trash-selection.ts:17` | should | yes |
| 95f58974 | unchanged-test | `bitwarden_license/bit-cli/src/service-container.ts:40` | needs no test | yes |
| 95f58974 | missing-test | `bitwarden_license/bit-web/src/app/secrets-manager/provide-secrets-manager.ts:24` | needs no test | yes |
| 09e5296b | unchanged-test | `apps/cli/src/admin-console/commands/confirm.command.ts:96` | needs no test | yes |
| 09e5296b | unchanged-test | `apps/web/src/app/admin-console/organizations/policies/policy-edit-definitions/organization-data-ownership.component.ts:131` | needs no test | yes |
| 09e5296b | unchanged-test | `libs/importer/src/services/import.service.ts:678` | needs no test | yes |
| 09e5296b | unchanged-test | `libs/vault/src/components/vault-items-table/vault-items-table.component.ts:601` | should | yes |
| 80507e69 | unchanged-test | `apps/desktop/src/app/accounts/settings-dialog.component.ts:848` | needs no test | yes |
| 7cb2858c | unchanged-test | `bitwarden_license/bit-web/src/app/pam/access-rules/access-rules.component.ts:133` | should | yes |
| 5f44fb5a | unchanged-test | `apps/web/src/app/tools/send/send-access/send-access-file.component.ts:31` | needs no test | yes |
| 5f44fb5a | unchanged-test | `apps/web/src/app/tools/send/send-access/send-view.component.ts:68` | needs no test | yes |
| 4d76bbfb | unchanged-test | `apps/browser/src/background/main.background.ts:629` | needs no test | yes |
| 4d76bbfb | unchanged-test | `apps/cli/src/service-container/service-container.ts:404` | needs no test | yes |
| 484911b2 | unchanged-test | `apps/desktop/src/main/native-messaging.main.ts:488` | should | yes |
| 8bb9d5b0 | unchanged-test | `apps/web/src/app/billing/trial-initiation/complete-trial-initiation/resolver/free-trial-text.resolver.ts:8` | should | yes |
| 6b8ce547 | unchanged-test | `apps/browser/src/background/main.background.ts:2323` | needs no test | yes |
| d8660243 | unchanged-test | `apps/browser/src/background/main.background.ts:628` | needs no test | yes |
| d8660243 | missing-test | `bitwarden_license/bit-browser/src/pam/lease-messages.ts:16` | should | yes |
| d8660243 | missing-test | `bitwarden_license/bit-browser/src/pam/leased-cipher.state.spec-util.ts:10` | test code | yes |
| d8660243 | missing-test | `bitwarden_license/bit-browser/src/pam/popup/popup-revealed-cipher.service.ts:17` | should | yes |
| d8660243 | missing-test | `bitwarden_license/bit-browser/src/pam/popup/provide-pam.ts:53` | needs no test | yes |
| 88fa0962 | unchanged-test | `apps/browser/src/background/main.background.ts:628` | needs no test | yes |
| 88fa0962 | unchanged-test | `apps/cli/src/service-container/service-container.ts:400` | needs no test | yes |

## server

| commit | rule | file | category | after |
|---|---|---|---|---|
| ebefe07a | unchanged-test | `src/Admin/AdminConsole/Controllers/OrganizationsController.cs:764` | should | yes |
| 04a4d754 | missing-test | `src/Admin/AdminConsole/Jobs/SendInviteRemindersJob.cs:16` | should | yes |
| c22f7be6 | missing-test | `bitwarden_license/src/Sso/Utilities/Saml2/NoopCookieManager.cs:26` | needs no test | yes |
| 8dbb66d8 | unchanged-test | `bitwarden_license/src/Sso/Controllers/AccountController.cs:68` | miss | no |
| 8dbb66d8 | missing-test | `src/Core/Billing/Organizations/Models/PrivilegedControlsSeatReservationResult.cs:17` | needs no test | yes |

## CleanArchitecture

| commit | rule | file | category | after |
|---|---|---|---|---|
| a6b76550 | unchanged-test | `src/Application/TodoItems/Commands/CreateTodoItem/CreateTodoItem.cs:22` | should | yes |
| a6b76550 | unchanged-test | `src/Application/TodoItems/Commands/DeleteTodoItem/DeleteTodoItem.cs:16` | should | yes |
| a6b76550 | unchanged-test | `src/Application/TodoItems/Commands/UpdateTodoItem/UpdateTodoItem.cs:23` | needs no test | yes |
| a6b76550 | unchanged-test | `src/Application/TodoItems/Commands/UpdateTodoItemDetail/UpdateTodoItemDetail.cs:26` | needs no test | yes |
| a6b76550 | unchanged-test | `src/Application/TodoLists/Commands/CreateTodoList/CreateTodoList.cs:23` | should | yes |
| a6b76550 | unchanged-test | `src/Application/TodoLists/Commands/UpdateTodoList/UpdateTodoList.cs:24` | should | yes |
| a6b76550 | unchanged-test | `src/Application/TodoLists/Queries/GetTodos/GetTodos.cs:23` | should | yes |
| 4b1f5a40 | missing-test | `src/Web/ClientApp/src/app/theme-toggle/theme-toggle.component.ts:10` | should | yes |
| 4b1f5a40 | missing-test | `src/Web/ClientApp/src/app/theme.service.ts:11` | should | yes |
| 4b1f5a40 | missing-test | `src/Web/ClientApp/src/app/weather/weather.component.ts:14` | should | yes |
| d4ac254a | missing-test | `src/Web/Infrastructure/IEndpointGroup.cs:16` | needs no test | no |
| d4ac254a | missing-test | `src/Web/Infrastructure/ProblemDetailsExceptionHandler.cs:15` | needs no test | no |
| 766a9335 | unchanged-test | `src/Web/ClientApp/src/api-authorization/authorize.interceptor.ts:13` | should | yes |
| adb6e205 | missing-test | `src/Web/ClientApp/src/api-authorization/auth.guard.ts:11` | should | yes |
| adb6e205 | missing-test | `src/Web/ClientApp/src/api-authorization/auth.service.ts:13` | should | yes |
| adb6e205 | missing-test | `src/Web/ClientApp/src/api-authorization/login/login.component.ts:15` | should | yes |
| adb6e205 | missing-test | `src/Web/ClientApp/src/api-authorization/register/register.component.ts:15` | should | yes |
| adb6e205 | missing-test | `src/Web/Infrastructure/IdentityApiOperationTransformer.cs:22` | needs no test | no |
| 7ce369fd | missing-test | `src/Web/Infrastructure/BearerSecuritySchemeTransformer.cs:9` | needs no test | no |

## eShop

| commit | rule | file | category | after |
|---|---|---|---|---|
| ae71a061 | missing-test | `e2e/cart-helpers.ts:3` | test code | no |
| b3b115bd | unchanged-test | `src/ClientApp/Services/Catalog/CatalogService.cs:26` | should | yes |

## Totals

| repository | should | needs no test | test code | miss |
|---|---|---|---|---|
| clients | 12 | 26 | 3 | 0 |
| server | 2 | 2 | 0 | 1 |
| CleanArchitecture | 13 | 6 | 0 | 0 |
| eShop | 1 | 0 | 1 | 0 |
