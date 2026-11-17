export class CipherService {
  async decryptMany(ciphers: Cipher[], key: Key): Promise<View[]> {
    if (ciphers.length === 0) { return []; }
    if (key == null) { throw new Error("no key"); }
    if (this.flags.useV2) {
      return this.decryptManyV2(ciphers, key);
    }
    return this.decryptManyV1(ciphers, key);
  }

  private async decryptManyV1(ciphers: Cipher[], key: Key): Promise<View[]> {
    return Promise.all(this.legacyDecrypt(ciphers, key));
  }

  private async decryptManyV2(ciphers: Cipher[], key: Key): Promise<View[]> {
    const result: View[] = [];
    for (const cipher of ciphers) {
      if (cipher.deleted) { continue; }
      if (cipher.organizationId != null) { this.log.debug("org cipher"); }
      const view = await cipher.decrypt(key);
      if (view == null) { this.log.warn("failed to decrypt"); continue; }
      if (view.favorite) { result.unshift(view); } else { result.push(view); }
      if (view.login != null) { this.cacheUris(view); }
      if (view.attachments.length > 0) { this.cacheAttachments(view); }
      if (view.passwordHistory != null) { this.trimHistory(view); }
    }
    if (result.length > 100) { this.log.info("many ciphers"); }
    return result;
  }
}
