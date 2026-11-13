# Spec Delta

## MODIFIED Requirements

### Requirement: Missing comment permission

When the build identity may not contribute to pull requests, the comment
step SHALL fail with a message that names the missing permission
("Contribute to pull requests") and the identity that needs it: the
project's Build Service when the token belongs to it, the Project
Collection Build Service when the token belongs to that, and both when
the error does not tell which. It SHALL NOT leave a partial comment.

#### Scenario: Permission not granted

- **GIVEN** the project's Build Service may not contribute to pull requests on the repository
- **WHEN** the documented job runs for a pull request with a finding
- **THEN** the comment step fails, and its log names "Contribute to pull requests" and the Build Service

#### Scenario: A token with collection scope

- **GIVEN** a pipeline whose job authorization scope is the project collection, and whose Project Collection Build Service may not contribute to pull requests
- **WHEN** the documented job runs for a pull request with a finding
- **THEN** the comment step fails, and its log names "Contribute to pull requests" and the Project Collection Build Service

#### Scenario: An identity the script cannot place

- **GIVEN** a permission error whose identity matches neither the project's nor the collection's id, or names no identity
- **WHEN** the comment step reports it
- **THEN** its log names "Contribute to pull requests", the project's Build Service and the Project Collection Build Service
