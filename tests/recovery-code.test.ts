import { describe, expect, it } from "vitest";
import { isValidRecoveryCode, normalizeRecoveryCode } from "../src/pages/Login";

/**
 * O código de recuperação passou de 8 para 16 bytes quando deixou de servir
 * apenas para redefinir a senha e passou a proteger uma cópia da chave de dados.
 *
 * A validação anterior era uma regex de exatamente 4 grupos de 4. Ela rejeitaria
 * um código novo válido antes mesmo de enviá-lo ao servidor — e o sintoma seria
 * "código inválido" num momento em que a profissional já perdeu a senha, com o
 * prontuário dos pacientes do outro lado.
 */
describe("código de recuperação", () => {
  it("aceita o formato novo de 128 bits (8 grupos)", () => {
    expect(isValidRecoveryCode("ABCD-EF01-2345-6789-ABCD-EF01-2345-6789")).toBe(true);
  });

  it("aceita o formato antigo de 64 bits, que usuários já têm anotado", () => {
    expect(isValidRecoveryCode("ABCD-EF01-2345-6789")).toBe(true);
  });

  it("aceita sem hífens e em minúsculas", () => {
    expect(isValidRecoveryCode("abcdef0123456789")).toBe(true);
    expect(isValidRecoveryCode("abcdef0123456789abcdef0123456789")).toBe(true);
    expect(isValidRecoveryCode("ABCD EF01 2345 6789")).toBe(true);
  });

  it("recusa tamanho intermediário, que não corresponde a nenhuma geração", () => {
    expect(isValidRecoveryCode("ABCD-EF01-2345")).toBe(false);
    expect(isValidRecoveryCode("ABCD-EF01-2345-6789-ABCD")).toBe(false);
  });

  it("recusa caractere que não é hexadecimal", () => {
    expect(isValidRecoveryCode("ZZZZ-EF01-2345-6789")).toBe(false);
    expect(isValidRecoveryCode("")).toBe(false);
  });

  it("normaliza igual ao backend: maiúsculas e sem separadores", () => {
    expect(normalizeRecoveryCode("abcd-ef01 2345_6789")).toBe("ABCDEF0123456789");
  });
});
