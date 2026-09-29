/** Public web origin for invite links we generate. Parsers also accept legacy `titi.app`. */
export const WEB_ORIGIN = "https://titi.dragoscatalin.ro";

export const toWebLink = (l: string) => l.replace("titi://j/", `${WEB_ORIGIN}/j/`);

export const isInviteUrl = (t: string) => t.startsWith("titi://") || /(^|\/\/)(titi\.dragoscatalin\.ro|titi\.app)\/j\//.test(t);
